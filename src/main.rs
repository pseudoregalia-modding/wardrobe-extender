use unreal_asset::{properties::*, types::PackageIndex, Export};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("accessing filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("parsing asset: {0}")]
    UnrealAsset(#[from] unreal_asset::Error),
    #[error("parsing save: {0}")]
    Gvas(#[from] gvas::error::Error),
    #[error("reading/writing pak: {0}")]
    Repak(#[from] repak::Error),
    #[error("couldn't cast DT_OutfitData - verify your game files")]
    Datatable,
    #[error("couldn't get costume pak name")]
    Filestem,
}

type Asset = unreal_asset::Asset<std::io::Cursor<Vec<u8>>>;

#[link(name = "oo2core_win64", kind = "static")]
extern "C" {
    fn OodleLZ_Decompress(
        compBuf: *mut u8,
        compBufSize: usize,
        rawBuf: *mut u8,
        rawLen: usize,
        fuzzSafe: u32,
        checkCRC: u32,
        verbosity: u32,
        decBufBase: u64,
        decBufSize: usize,
        fpCallback: u64,
        callbackUserData: u64,
        decoderMemory: *mut u8,
        decoderMemorySize: usize,
        threadPhase: u32,
    ) -> i32;
}

fn main() {
    loop {
        match run() {
            Ok(()) => break,
            Err(e) => {
                eprintln!("{e}");
                println!("press enter to try again :/");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
        }
    }
}

fn run() -> Result<(), Error> {
    let pak = || std::fs::File::open("pseudoregalia-Windows.pak");
    let game = repak::PakBuilder::new()
        .oodle(|| OodleLZ_Decompress)
        .reader_with_version(&mut pak()?, repak::Version::V11)?;
    std::fs::create_dir_all("outfits")?;
    std::fs::create_dir_all("~mods")?;
    let mut pak = pak()?;
    let mut get = |path: &str| -> Result<Asset, Error> {
        Ok(unreal_asset::Asset::new(
            std::io::Cursor::new(game.get(&(path.to_string() + ".uasset"), &mut pak)?),
            Some(std::io::Cursor::new(
                game.get(&(path.to_string() + ".uexp"), &mut pak)?,
            )),
            unreal_asset::engine_version::EngineVersion::VER_UE5_1,
            None,
        )?)
    };
    let mut table_asset = get("pseudoregalia/Content/Data/DataTables/DT_OutfitData")?;
    let mut table_names = table_asset.get_name_map();
    let table = &mut unreal_asset::cast!(
        Export,
        DataTableExport,
        &mut table_asset.asset_data.exports[0]
    )
    .ok_or(Error::Datatable)?
    .table
    .data;
    let mut outfits = vec![];
    let mut modfiles = repak::PakBuilder::new().writer(
        std::fs::File::create("~mods/outfits_p.pak")?,
        repak::Version::V11,
        "../../../".to_string(),
        None,
    );
    for outfit in std::fs::read_dir("outfits")?
        .map(|entry| Ok::<_, Error>(entry?.path()))
        .filter(|entry| {
            entry
                .as_ref()
                .is_ok_and(|entry| entry.extension() == Some(std::ffi::OsStr::new("pak")))
        })
        .map(|entry| {
            let entry = entry?;
            Ok::<_, Error>((
                entry
                    .file_stem()
                    .ok_or(Error::Filestem)?
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
                repak::PakBuilder::new().reader(&mut std::fs::File::open(&entry)?)?,
                std::fs::File::open(&entry)?,
            ))
        })
    {
        let (costume_name, pak, mut file) = outfit?;
        let mut table_names = table_names.get_mut();
        let path = "pseudoregalia/Content/Meshes/Characters/".to_string() + &costume_name;
        let mount = pak.mount_point().trim_start_matches("../../../");
        for asset in pak.files() {
            modfiles.write_file(
                &(mount.to_string() + &asset),
                &mut pak.get(&asset, &mut file)?,
            )?;
        }
        outfits.push(gvas::properties::Property::NameProperty(
            gvas::properties::name_property::NameProperty {
                value: costume_name.clone(),
            },
        ));
        table_asset.imports.push(unreal_asset::Import {
            class_package: table_names.add_fname("/Script/CoreUObject"),
            class_name: table_names.add_fname("Package"),
            outer_index: PackageIndex::new(0),
            object_name: table_names.add_fname(&path.replace("pseudoregalia/Content", "/Game")),
            optional: false,
        });
        table_asset.imports.push(unreal_asset::Import {
            class_package: table_names.add_fname("/Script/Engine"),
            class_name: table_names.add_fname("SkeletalMesh"),
            outer_index: PackageIndex::new(-(table_asset.imports.len() as i32)),
            object_name: table_names.add_fname(&costume_name),
            optional: false,
        });
        table.push(struct_property::StructProperty {
            name: table_names.add_fname(&costume_name),
            value: vec![
                Property::TextProperty(str_property::TextProperty {
                    name: table_names.add_fname("OutfitName_8_30C4367C4FD7CFAC4EBE87A1AE15FA90"),
                    culture_invariant_string: Some(costume_name.replace("_", " ")),
                    ancestry: Default::default(),
                    property_guid: Some(0.into()),
                    duplication_index: Default::default(),
                    namespace: Default::default(),
                    table_id: Default::default(),
                    flags: Default::default(),
                    history_type: Default::default(),
                    value: Default::default(),
                }),
                Property::ObjectProperty(object_property::ObjectProperty {
                    name: table_names.add_fname("SkeletalMesh_12_21A9339348FA07AF7351F1BCBE3768FA"),
                    value: PackageIndex::new(-(table_asset.imports.len() as i32)),
                    property_guid: Some(0.into()),
                    ..Default::default()
                }),
                Property::ArrayProperty(array_property::ArrayProperty {
                    name: table_names.add_fname("Description_11_D997B7CD46E0BEE9A35AD7BB3DC71F91"),
                    array_type: Some(table_names.add_fname("TextProperty")),
                    property_guid: Some(0.into()),
                    ..Default::default()
                }),
            ],
            property_guid: Some(0.into()),
            ..Default::default()
        });
        println!("{} added", costume_name.replace("_", " "));
    }
    let mut table = (std::io::Cursor::new(vec![]), std::io::Cursor::new(vec![]));
    table_asset.write_data(&mut table.0, Some(&mut table.1))?;
    modfiles.write_file(
        "pseudoregalia/Content/Data/DataTables/DT_OutfitData.uasset",
        table.0.into_inner(),
    )?;
    modfiles.write_file(
        "pseudoregalia/Content/Data/DataTables/DT_OutfitData.uexp",
        table.1.into_inner(),
    )?;
    modfiles.write_index()?;

    let Some(saves) = std::env::var_os("USERPROFILE")
        .filter(|home| !home.is_empty())
        .map(std::path::PathBuf::from)
        .map(|path| path.join("AppData/Local/pseudoregalia/Saved/SaveGames"))
    else {
        return Ok(());
    };
    for bundle in saves
        .read_dir()?
        .map(|entry| Ok::<_, Error>(entry?.path()))
        .filter(|entry| {
            entry.as_ref().is_ok_and(|entry| {
                entry.extension() == Some(std::ffi::OsStr::new("sav"))
                    && entry
                        .file_name()
                        .is_some_and(|file| file.to_str().unwrap_or_default().starts_with("File "))
            })
        })
        .map(|entry| {
            let entry = entry?;
            Ok::<_, Error>((
                gvas::GvasFile::read(&mut std::fs::File::open(&entry)?)?,
                entry,
            ))
        })
    {
        let (mut save, path) = bundle?;
        let Some(unlocked) = save
            .properties
            .get_mut("unlockedOutfits")
            .and_then(gvas::properties::Property::get_array_mut)
        else {
            println!(
                "{:?} skipped - resave this file in-game to fix",
                path.file_name().unwrap_or_default()
            );
            continue;
        };
        unlocked.properties.extend_from_slice(&outfits);
        unlocked.properties.dedup();
        if let Some(current) = save
            .properties
            .get_mut("currentOutfit")
            .and_then(gvas::properties::Property::get_name_mut)
        {
            current.value = "base".to_string()
        }
        save.write(&mut std::fs::File::create(&path)?)?;
        println!("{:?} written", path.file_name().unwrap_or_default());
    }
    println!("finished! you can now launch the game");
    println!("press enter to exit :)");
    std::io::stdin().read_line(&mut String::new())?;
    Ok(())
}
