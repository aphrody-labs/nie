//! Loads assets through Bevy's `AssetServer` from a generated synthetic game: an encrypted
//! `cpk_list.cfg.bin`, a real CPK archive and a PNG, all built in a temp directory at test time.
//! No game byte is read or written, so the test runs anywhere, headless.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{
    Asset, AssetApp, AssetMetaCheck, AssetPlugin, AssetServer, Assets, Handle, LoadState,
};
use bevy_image::{CompressedImageFormats, Image, ImageLoader};
use nie_assets::{AssetsConfig, GameAssetsPlugin, GameBytes, GameBytesLoader};
use nie_formats::cfgbin::{CfgEntry, Value, encode_t2b};
use nie_formats::cpk::encrypt_cpk_list;
use nie_formats::cpk_encode::{CpkWriteEntry, encode_cpk};
use nie_formats::vfs::Vfs;

const HELLO: &[u8] = b"synthetic text asset";

fn blob() -> Vec<u8> {
    (0..=255u8).collect()
}

fn pixel_png() -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 2, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ])
        .unwrap();
    writer.finish().unwrap();
    out
}

/// Builds `<root>/data/{cpk_list.cfg.bin,packs/synthetic.cpk}` and returns `<root>`.
fn build_game(root: &Path) {
    let files: [(&str, &str, Vec<u8>); 3] = [
        ("data/common", "hello.txt", HELLO.to_vec()),
        ("data/common", "blob.bin", blob()),
        ("data/common/tex", "pixel.png", pixel_png()),
    ];
    let packs = root.join("data/packs");
    std::fs::create_dir_all(&packs).unwrap();
    let archive = encode_cpk(
        &files
            .iter()
            .map(|(dir, name, data)| CpkWriteEntry {
                directory: (*dir).to_owned(),
                filename: (*name).to_owned(),
                data: data.clone(),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    std::fs::write(packs.join("synthetic.cpk"), archive).unwrap();

    let children: Vec<CfgEntry> = files
        .iter()
        .map(|(dir, name, data)| CfgEntry {
            name: "CPK_ITEM".to_owned(),
            variables: vec![
                Value::String(format!("{dir}/")),
                Value::String((*name).to_owned()),
                Value::String(String::new()),
                Value::String("synthetic.cpk".to_owned()),
                Value::Int(i32::try_from(data.len()).unwrap()),
            ],
            children: Vec::new(),
        })
        .collect();
    let list = CfgEntry {
        name: "CPK_LIST".to_owned(),
        variables: vec![Value::Int(i32::try_from(children.len()).unwrap())],
        children,
    };
    std::fs::write(
        root.join("data/cpk_list.cfg.bin"),
        encrypt_cpk_list(&encode_t2b(&[list])),
    )
    .unwrap();
}

fn headless_app(root: &Path, cache: Option<&Path>) -> App {
    let mut config = AssetsConfig::for_source(root.to_string_lossy());
    config.cache_dir = cache.map(Path::to_path_buf);
    let plugin = GameAssetsPlugin::new(&config).expect("mount synthetic game");
    let mut app = App::new();
    // The source must be registered before `AssetPlugin`.
    app.add_plugins(plugin);
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin {
            meta_check: AssetMetaCheck::Never,
            ..Default::default()
        },
    ));
    app.init_asset::<GameBytes>()
        .register_asset_loader(GameBytesLoader)
        .init_asset::<Image>()
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));
    app
}

fn drive<A: Asset>(app: &mut App, handle: &Handle<A>) {
    let started = Instant::now();
    loop {
        app.update();
        let server = app.world().resource::<AssetServer>();
        match server.load_state(handle) {
            LoadState::Loaded => return,
            LoadState::Failed(error) => panic!("asset failed to load: {error}"),
            _ => {}
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "asset load timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn bytes_and_texture_load_through_the_asset_server_from_a_cpk() {
    let game = tempfile::tempdir().unwrap();
    build_game(game.path());
    let mut app = headless_app(game.path(), None);

    let server = app.world().resource::<AssetServer>().clone();
    let bytes: Handle<GameBytes> = server.load("game://data/common/blob.bin");
    let image: Handle<Image> = server.load("game://data/common/tex/pixel.png");
    drive(&mut app, &bytes);
    drive(&mut app, &image);

    let assets = app.world().resource::<Assets<GameBytes>>();
    assert_eq!(assets.get(&bytes).unwrap().0, blob());
    let images = app.world().resource::<Assets<Image>>();
    let image = images.get(&image).unwrap();
    assert_eq!((image.width(), image.height()), (2, 2));
}

#[test]
fn missing_asset_fails_and_directories_enumerate() {
    let game = tempfile::tempdir().unwrap();
    build_game(game.path());
    let mut vfs = Vfs::new();
    vfs.init(game.path().join("data")).unwrap();
    let vfs = Arc::new(vfs);
    assert_eq!(vfs.read("data/common/hello.txt").unwrap(), HELLO);

    let mut app = headless_app(game.path(), None);
    let server = app.world().resource::<AssetServer>().clone();
    let missing: Handle<GameBytes> = server.load("game://data/common/absent.bin");
    let started = Instant::now();
    loop {
        app.update();
        if matches!(server.load_state(&missing), LoadState::Failed(_)) {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "failure not reported"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn disk_cache_serves_a_file_after_the_pack_is_gone() {
    let game = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    build_game(game.path());
    let mut app = headless_app(game.path(), Some(cache.path()));
    let server = app.world().resource::<AssetServer>().clone();

    let first: Handle<GameBytes> = server.load("game://data/common/blob.bin");
    drive(&mut app, &first);
    assert!(
        std::fs::read_dir(cache.path()).unwrap().next().is_some(),
        "file cached"
    );

    // A second app over the same cache: the archive is gone, the cached file still loads.
    std::fs::remove_file(game.path().join("data/packs/synthetic.cpk")).unwrap();
    let mut second = headless_app(game.path(), Some(cache.path()));
    let server = second.world().resource::<AssetServer>().clone();
    let again: Handle<GameBytes> = server.load("game://data/common/blob.bin");
    drive(&mut second, &again);
    let assets = second.world().resource::<Assets<GameBytes>>();
    assert_eq!(assets.get(&again).unwrap().0, blob());
}
