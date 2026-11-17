//! End-to-end scenario on a real PostgreSQL. Set `ACHILLEA_TEST_DATABASE_URL` (a disposable
//! database); without it the test reports a skip and passes.

use achillea_store::*;
use tokio_postgres::NoTls;

async fn connect() -> Option<tokio_postgres::Client> {
    let url = std::env::var("ACHILLEA_TEST_DATABASE_URL").ok()?;
    let (client, conn) = tokio_postgres::connect(&url, NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = conn.await;
    });
    Some(client)
}

#[tokio::test]
async fn full_scenario() {
    let Some(mut c) = connect().await else {
        eprintln!("SKIPPED: ACHILLEA_TEST_DATABASE_URL is not set");
        return;
    };
    c.batch_execute("DROP SCHEMA IF EXISTS achillea CASCADE")
        .await
        .unwrap();
    migrate(&c).await.unwrap();
    migrate(&c).await.expect("migration is idempotent");

    // ---- ranked
    let season = create_season(&c, "s1", 32, 1000).await.unwrap();
    let mut p = Vec::new();
    for i in 0..8 {
        p.push(
            upsert_player(&c, &format!("d{i}"), &format!("player{i}"))
                .await
                .unwrap(),
        );
    }
    let m = record_ranked_match(&mut c, season, 2, p[0], p[1], 3.0, 1.0)
        .await
        .unwrap();
    assert_eq!(
        (m.home_delta, m.away_delta, m.home_ap, m.away_ap),
        (20, -5, 1020, 995),
        "tier 0, equal rating: +20 / -5"
    );
    let m2 = record_ranked_match(&mut c, season, 2, p[0], p[1], 2.0, 2.0)
        .await
        .unwrap();
    assert_eq!(
        m2.home_ap + m2.away_ap,
        m2.home_delta + m2.away_delta + 1020 + 995,
        "ratings follow the deltas"
    );
    assert!(
        record_ranked_match(&mut c, season, 2, p[0], p[0], 1.0, 0.0)
            .await
            .is_err()
    );
    assert!(
        record_ranked_match(&mut c, season, 3, p[0], p[1], 1.0, 0.0)
            .await
            .is_err()
    );
    let lb = leaderboard(&c, season, 10).await.unwrap();
    assert_eq!(lb[0].username, "player0");
    assert!(lb[0].ap > lb[1].ap);

    // ---- clans
    let clan = create_clan(&mut c, "Griffons", "RG", p[0]).await.unwrap();
    join_clan(&c, clan, p[1]).await.unwrap();
    join_clan(&c, clan, p[2]).await.unwrap();
    assert!(
        join_clan(&c, clan, p[1]).await.is_err(),
        "one clan per player"
    );
    set_member_role(&mut c, clan, p[0], p[1], 2).await.unwrap();
    assert!(
        set_member_role(&mut c, clan, p[1], p[2], 2).await.is_err(),
        "cannot grant own rank"
    );
    assert!(
        set_member_role(&mut c, clan, p[2], p[1], 0).await.is_err(),
        "a trial cannot act"
    );
    let members = clan_members(&c, clan).await.unwrap();
    assert_eq!(
        members.iter().map(|m| m.1).collect::<Vec<_>>(),
        vec![3, 2, 0]
    );

    // ---- tournament (6 players in an 8 bracket: seeds 1 and 2 get byes)
    let t = create_tournament(&c, season, "Evo", 6).await.unwrap();
    for id in &p[..6] {
        register_player(&c, t, *id).await.unwrap();
    }
    assert_eq!(start_tournament(&mut c, t).await.unwrap(), 6);
    assert!(
        register_player(&c, t, p[6]).await.is_err(),
        "registration closed"
    );
    let before = leaderboard(&c, season, 10).await.unwrap();
    for round in 1..=3 {
        let ids: Vec<i64> = c
            .query("SELECT id FROM achillea.tournament_match WHERE tournament_id = $1 AND round = $2 AND winner_id IS NULL AND home_player_id IS NOT NULL AND away_player_id IS NOT NULL ORDER BY slot", &[&t, &round])
            .await
            .unwrap()
            .iter()
            .map(|r| r.get(0))
            .collect();
        for id in ids {
            report_tournament_match(&mut c, id, 2, 0).await.unwrap();
        }
    }
    let state: String = c
        .query_one("SELECT state FROM achillea.tournament WHERE id = $1", &[&t])
        .await
        .unwrap()
        .get(0);
    assert_eq!(state, "finished");
    let placements: Vec<(i32, i64)> = c
        .query("SELECT placement, count(*) FROM achillea.tournament_entry WHERE tournament_id = $1 AND placement IS NOT NULL GROUP BY placement ORDER BY placement", &[&t])
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    assert_eq!(placements, vec![(1, 1), (2, 1), (3, 2)]);
    let after = leaderboard(&c, season, 10).await.unwrap();
    let gained: i32 =
        after.iter().map(|r| r.ap).sum::<i32>() - before.iter().map(|r| r.ap).sum::<i32>();
    // 6 players: first 25 + second 12 + two semi-finalists 5 each, plus base rating for new rows.
    let new_rows = i32::try_from(after.len() - before.len()).unwrap();
    assert_eq!(gained, 25 + 12 + 5 + 5 + 1000 * new_rows);

    // ---- league
    let l = create_league(&c, "Asytral", "asytral", false)
        .await
        .unwrap();
    for t in ["a", "b", "c", "d"] {
        add_league_team(&c, l, t, &t.to_uppercase(), None)
            .await
            .unwrap();
    }
    assert_eq!(generate_league_fixtures(&mut c, l).await.unwrap(), 6);
    let fx: Vec<i64> = c
        .query(
            "SELECT id FROM achillea.league_match WHERE league_id = $1 ORDER BY id",
            &[&l],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    for id in fx {
        // home always wins 2-1 in regulation
        report_league_match(&c, id, 2, 1, false).await.unwrap();
    }
    let table = league_standings(&c, l).await.unwrap();
    assert_eq!(table.len(), 4);
    assert_eq!(table.iter().map(|r| r.played).sum::<i32>(), 12);
    assert_eq!(
        table.iter().map(|r| r.points).sum::<i32>(),
        6 * 3,
        "6 matches, 3 points each"
    );
    assert_eq!(table[0].rank, 1);

    // ---- Ultimate Team
    set_rarities(&c, &[(0, "common", 80), (1, "rare", 20)])
        .await
        .unwrap();
    c.batch_execute("CREATE SCHEMA IF NOT EXISTS gamedata; DROP TABLE IF EXISTS gamedata.chara; CREATE TABLE gamedata.chara (row_index INTEGER PRIMARY KEY, chara_id BIGINT, name TEXT); INSERT INTO gamedata.chara VALUES (0, 10, 'Axel'), (1, 11, 'Mark'), (2, 12, 'Jude')").await.unwrap();
    assert_eq!(
        import_cards(
            &c,
            "gamedata",
            "chara",
            "chara_id",
            "name",
            0,
            "data/x.cfg.bin"
        )
        .await
        .unwrap(),
        3
    );
    assert_eq!(
        import_cards(
            &c,
            "gamedata",
            "chara",
            "chara_id",
            "name",
            1,
            "data/y.cfg.bin"
        )
        .await
        .unwrap(),
        3
    );
    let a = open_pack(&mut c, p[0], 5, 42).await.unwrap();
    let b = open_pack(&mut c, p[1], 5, 42).await.unwrap();
    assert_eq!(a, b, "same seed, same pack");
    let mut differs = false;
    for seed in 43..60 {
        differs |= a != open_pack(&mut c, p[2], 5, seed).await.unwrap();
    }
    assert!(differs, "different seeds give different packs");
    let owned: i64 = c
        .query_one(
            "SELECT coalesce(sum(quantity), 0) FROM achillea.ut_collection WHERE player_id = $1",
            &[&p[0]],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(owned, 5);
    c.batch_execute("DROP SCHEMA gamedata CASCADE")
        .await
        .unwrap();
}
