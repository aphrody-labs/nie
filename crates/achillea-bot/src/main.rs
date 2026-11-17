//! `achillea-bot`: reads `DISCORD_TOKEN`, `ACHILLEA_DATABASE_URL` and `ACHILLEA_SEASON` (season name,
//! created with the default 32 K-factor and base rating 0 when missing).

use std::sync::Arc;

use achillea_bot::{PendingReports, ReportResult};
use achillea_store as store;
use poise::serenity_prelude as serenity;
use tokio::sync::Mutex;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

struct Data {
    db: Arc<Mutex<tokio_postgres::Client>>,
    season: i32,
    pending: Mutex<PendingReports>,
}

async fn player_id(ctx: Context<'_>, user: &serenity::User) -> Result<i64, Error> {
    let db = ctx.data().db.lock().await;
    Ok(store::upsert_player(&db, &user.id.to_string(), &user.name).await?)
}

/// Report your score against an opponent; the match counts when both of you report the same result.
#[poise::command(slash_command)]
async fn report(
    ctx: Context<'_>,
    #[description = "Your opponent"] opponent: serenity::User,
    #[description = "Your score"] mine: u32,
    #[description = "Their score"] theirs: u32,
) -> Result<(), Error> {
    let me = ctx.author().clone();
    if me.id == opponent.id {
        ctx.say("You cannot play against yourself.").await?;
        return Ok(());
    }
    let result = ctx.data().pending.lock().await.report(
        me.id.get(),
        opponent.id.get(),
        f64::from(mine),
        f64::from(theirs),
    );
    match result {
        ReportResult::Waiting => {
            ctx.say(format!("Report saved, waiting for {}.", opponent.name))
                .await?
        }
        ReportResult::Dispute => {
            ctx.say("The two reports disagree. Both were cleared; report again.")
                .await?
        }
        ReportResult::Record {
            home,
            away,
            home_score,
            away_score,
        } => {
            let (h, a) = if home == me.id.get() {
                (&me, &opponent)
            } else {
                (&opponent, &me)
            };
            let (hid, aid) = (player_id(ctx, h).await?, player_id(ctx, a).await?);
            let mut db = ctx.data().db.lock().await;
            let out = store::record_ranked_match(
                &mut db,
                ctx.data().season,
                2,
                hid,
                aid,
                f64::from(home_score),
                f64::from(away_score),
            )
            .await?;
            let _ = away;
            ctx.say(format!(
                "{} {}-{} {}: {} AP ({:+}), {} AP ({:+}).",
                h.name,
                home_score,
                away_score,
                a.name,
                out.home_ap,
                out.home_delta,
                out.away_ap,
                out.away_delta
            ))
            .await?
        }
    };
    Ok(())
}

/// Top of the ladder.
#[poise::command(slash_command)]
async fn ladder(ctx: Context<'_>) -> Result<(), Error> {
    let rows = {
        let db = ctx.data().db.lock().await;
        store::leaderboard(&db, ctx.data().season, 10).await?
    };
    let text = rows
        .iter()
        .map(|r| {
            format!(
                "{}. {} - {} AP ({}W {}L)",
                r.rank, r.username, r.ap, r.wins, r.losses
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    ctx.say(if text.is_empty() {
        "No ranked player yet.".into()
    } else {
        text
    })
    .await?;
    Ok(())
}

/// Create a clan.
#[poise::command(slash_command)]
async fn clan_create(ctx: Context<'_>, name: String, tag: String) -> Result<(), Error> {
    let me = player_id(ctx, ctx.author()).await?;
    let mut db = ctx.data().db.lock().await;
    match store::create_clan(&mut db, &name, &tag, me).await {
        Ok(id) => {
            ctx.say(format!("Clan {name} [{tag}] created (#{id})."))
                .await?
        }
        Err(e) => ctx.say(format!("Could not create the clan: {e}")).await?,
    };
    Ok(())
}

/// Open an Ultimate Team pack.
#[poise::command(slash_command)]
async fn pack(ctx: Context<'_>) -> Result<(), Error> {
    let me = player_id(ctx, ctx.author()).await?;
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos() as u64
        ^ ctx.author().id.get();
    let mut db = ctx.data().db.lock().await;
    match store::open_pack(&mut db, me, 5, seed).await {
        Ok(ids) => ctx.say(format!("Pack opened: cards {ids:?}")).await?,
        Err(e) => ctx.say(format!("No pack: {e}")).await?,
    };
    Ok(())
}

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN");
    let url = std::env::var("ACHILLEA_DATABASE_URL").expect("ACHILLEA_DATABASE_URL");
    let season_name = std::env::var("ACHILLEA_SEASON").unwrap_or_else(|_| "season-1".into());
    let (client, conn) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
        .await
        .expect("connect to PostgreSQL");
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("postgres connection ended: {e}");
        }
    });
    store::migrate(&client).await.expect("migrate");
    let season = match client
        .query_opt(
            "SELECT id FROM achillea.season WHERE name = $1",
            &[&season_name],
        )
        .await
        .expect("season lookup")
    {
        Some(r) => r.get(0),
        None => store::create_season(&client, &season_name, 32, 0)
            .await
            .expect("create season"),
    };
    let db = Arc::new(Mutex::new(client));
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![report(), ladder(), clan_create(), pack()],
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    db,
                    season,
                    pending: Mutex::new(PendingReports::default()),
                })
            })
        })
        .build();
    let mut bot = serenity::ClientBuilder::new(token, serenity::GatewayIntents::non_privileged())
        .framework(framework)
        .await
        .expect("client");
    bot.start().await.expect("gateway");
}
