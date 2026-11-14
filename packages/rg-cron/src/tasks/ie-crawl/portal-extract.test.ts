/**
 * @license
 * Copyright 2026 Rose Griffon
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, expect, test } from "bun:test";
import {
  extraireCrossPortal,
  extrairePatchNoteDetail,
  extraireRePortal,
  extraireSeriesPortal,
  normalizeDotDate,
  normalizeIsoDate,
} from "./portal-extract";

describe("portal-extract", () => {
  test("normalizeDotDate et normalizeIsoDate unifient les formats de dates", () => {
    expect(normalizeDotDate("2025.12.6 [JST]")).toBe("2025.12.06");
    expect(normalizeDotDate("2026/11/12")).toBe("2026.11.12");
    expect(normalizeDotDate("2026年9月10日")).toBe("2026.09.10");
    expect(normalizeIsoDate("2026/11/12")).toBe("2026-11-12");
  });

  test("extraireCrossPortal détecte la sortie, les news et les fiches VR", () => {
    const homeHtml = `
      <html><body>
        <section class="firstview">
          <p>好評配信中！</p>
          <a href="https://store.inazuma-cross.jp/">Web Store</a>
          <a href="/guide/">Guide</a>
          <a href="/victory-road-2026/">VR Collab</a>
        </section>
        <ul class="news-list">
          <li><a href="/victory-road-2026/">2026/11/12 『イナズマイレブン 英雄たちのヴィクトリーロード』コラボ開催決定！</a></li>
          <li><a href="/guide/">2026/7/31 初心者ガイド公開！</a></li>
        </ul>
      </body></html>
    `;
    const guideHtml = `
      <html><body>
        <div class="point"><h3 class="point-title">POINT 01</h3><p>自分だけのイレブンを組もう</p></div>
      </body></html>
    `;
    const vrHtml = `
      <html><body>
        <section class="kv">
          <img src="/assets/img/vr/kv.webp" alt="KV" />
          <p>開催期間：2026.11.17 〜 2027.03.31</p>
        </section>
        <section class="story">コラボオリジナルストーリー STORY あらすじ 雲明たちがクロスに登場！</section>
        <section class="character">
          <ul>
            <li>
              <p class="profile-name">笹波 雲明</p>
              <span class="pos">MF</span>
              <p>CV：小村将</p>
              <img src="/assets/img/vr/chara-thumb-01.webp" alt="笹波 雲明" />
              <img src="/assets/img/vr/quality100/chara-01.webp" alt="笹波 雲明" />
              <img src="/assets/img/vr/hissatsu-01.webp" alt="必殺技 タクティカルステップ" />
              <p>フットボールへの情熱を秘めた少年。</p>
            </li>
          </ul>
        </section>
      </body></html>
    `;

    const res = extraireCrossPortal(homeHtml, guideHtml, vrHtml);
    expect(res.phase).toBe("released");
    expect(res.phaseLabelJa).toBe("好評配信中！");
    expect(res.storeUrl).toBe("https://store.inazuma-cross.jp/");
    expect(res.guideUrl).toBe("https://www.inazuma-cross.jp/guide/");
    expect(res.vrCollabUrl).toBe(
      "https://www.inazuma-cross.jp/victory-road-2026/",
    );
    expect(res.news).toHaveLength(2);
    expect(res.news[0]?.date).toBe("2026-11-12");
    expect(res.news[0]?.tagJa).toBe("COLLABORATION");
    expect(res.news[1]?.date).toBe("2026-07-31");
    expect(res.guidePoints?.[0]?.titleJa).toBe("POINT 01");
    expect(res.vrCollab?.period).toBe("2026.11.17 - 2027.03.31");
    expect(res.vrCollab?.characters[0]?.nameJa).toBe("笹波 雲明");
    expect(res.vrCollab?.characters[0]?.hissatsuNameJa).toBe(
      "タクティカルステップ",
    );
  });

  test("extrairePatchNoteDetail extrait date normalisée et HTML nettoyé", () => {
    const html = `
      <html><head><title>Update Ver. 7.2.2 | Patchnote</title></head>
      <body>
        <article class="patchnote">
          <div class="dateBox">2026.11.12 [JST]</div>
          <div class="detail">
            <p class="lead">Online Season 1 corrections</p>
            <img src="/victory-road/assets/img/sample.png" />
            <a href="/victory-road/en/topics/260910_01/">Topic</a>
            <script>alert(1)</script>
          </div>
        </article>
      </body></html>
    `;
    const detail = extrairePatchNoteDetail(
      html,
      "https://www.inazuma.jp/victory-road/patchnote/switch/ver_7_2_2/",
      "Ver.7.2.2",
    );
    expect(detail.date).toBe("2026.11.12");
    expect(detail.title).toBe("Ver.7.2.2");
    expect(detail.contentHtml).toContain(
      "https://www.inazuma.jp/victory-road/assets/img/sample.png",
    );
    expect(detail.contentHtml).not.toContain("<script>");
  });

  test("extraireRePortal et extraireSeriesPortal parsent les métadonnées officielles", () => {
    const reHtml = `
      <html><head><title>INAZUMA ELEVEN RE</title></head><body>
        <dl>
          <dt>Title</dt><dd>INAZUMA ELEVEN RE</dd>
          <dt>Supported Platform</dt><dd>Nintendo Switch 2 / Nintendo Switch / PlayStation®5 / Steam</dd>
          <dt>Release Date</dt><dd>Scheduled for Release in 2026</dd>
        </dl>
        <img src="/re/assets/img/img_pv_pc.jpg" />
      </body></html>
    `;
    const re = extraireRePortal(reHtml);
    expect(re.title).toBe("INAZUMA ELEVEN RE");
    expect(re.platforms).toContain("Nintendo Switch 2");
    expect(re.releaseDate).toContain("2026");
    expect(re.teaserImageUrl).toBe(
      "https://www.inazuma.jp/re/assets/img/img_pv_pc.jpg",
    );

    const seriesHtml = `
      <html><body><ul class="gameList">
        <li><a href="/victory-road/"><img src="/assets/img/top/img_victory-road.jpg" alt="イナズマイレブン 英雄たちのヴィクトリーロード" />Switch/PS5</a></li>
        <li><a href="/re/"><img src="/assets/img/top/img_inazuma-re.jpg" alt="イナズマイレブン RE" />Switch 2</a></li>
      </ul></body></html>
    `;
    const series = extraireSeriesPortal(seriesHtml);
    expect(series).toHaveLength(2);
    expect(series[0]?.href).toBe("https://www.inazuma.jp/victory-road/");
    expect(series[1]?.thumbnailUrl).toBe(
      "https://www.inazuma.jp/assets/img/top/img_inazuma-re.jpg",
    );
  });
});
