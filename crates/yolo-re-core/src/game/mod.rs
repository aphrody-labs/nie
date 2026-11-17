// SPDX-License-Identifier: Apache-2.0
//! Game RE façade over [`rg_re::game`].

pub use rg_re::game::{
    catalog, error, formats, fs, gameplay, inventory, live, reverse, steam, text, unified, world,
};

pub use catalog::{
    CANON_STATUS_DERIVED_GAME, DetectedGame, DragonBallGame, GameEngine, Platform, SaveLocation,
    filter_by_engine, filter_by_platform, find_game_by_android_package, find_game_by_id,
    find_game_by_steam_app_id, get_dragon_ball_catalog, scan_all_installed_games,
    scan_installed_android_packages, scan_installed_steam_games, search_catalog,
};
pub use error::{ReError, Result};
pub use formats::{
    bac::{BacFile, BacHitbox, BacMove},
    bdm::{BdmEntry, BdmFile, DamageType},
    cms::{CmsEntry, CmsFile},
    cus::{CusFile, CusSkill, SkillCategory},
    emb::{EmbFile, EmbRgbaTexture, EmbTextureEntry, TexturePixelFormat},
    emd::{EmdFile, EmdMesh, EmdModelPart, EmdSubmesh, EmdVertex},
    emm::{EmmFile, EmmMaterial, EmmParameter},
    esk::{EskBone, EskFile},
    model_assemble::{AssembledCharacterModel, ModelPrimitive},
    moveset::{CharacterMoveset, MoveFrameData},
    msg::{MsgEntry, MsgFile},
    pup::{PupEntry, PupFile},
    sav::{Dxbv2Cac, Dxbv2Save},
};
pub use live::aob::AobPattern;
pub use steam::{
    Dbxv2Install, STEAM_APP_ID_DB_FIGHTERZ, STEAM_APP_ID_DB_KAKAROT, STEAM_APP_ID_DB_SPARKING_ZERO,
    STEAM_APP_ID_DBXV1, STEAM_APP_ID_DBXV2, SteamGameInstall, inspect_dbxv2_install,
    scan_dragon_ball_installs,
};
pub use unified::{
    BinaryFunctionEntry, BinaryReOverview, Dbxv2UnifiedState, LiveWineOverview, ResolvedSkill,
    SaveOverview, UnifiedCharacterProfile,
};
pub use world::xenoverse3::{Xenoverse3Map, xenoverse3_prototype_map};
