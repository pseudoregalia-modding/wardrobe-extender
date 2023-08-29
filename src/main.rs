use unreal_asset::{properties::*, types::PackageIndex, Export};

fn main() {
    // luckily pseudoregalia is the first version it checks but again need to have a chat with truman about api
    let pak = || std::fs::File::open("pseudoregalia-Windows.pak").unwrap();
    // new_any should really take a mutable reference to the reader since it doesn't store it
    let game = repak::PakReader::new_any(&mut pak()).unwrap();
    std::fs::create_dir_all("outfits").unwrap();
    std::fs::create_dir_all("~mods").unwrap();
    let mut pak = pak();
    let mut get = |path: &str| {
        unreal_asset::Asset::new(
            std::io::Cursor::new(game.get(&(path.to_string() + ".uasset"), &mut pak).unwrap()),
            Some(std::io::Cursor::new(
                game.get(&(path.to_string() + ".uexp"), &mut pak).unwrap(),
            )),
            unreal_asset::engine_version::EngineVersion::VER_UE5_1,
            None,
        )
        .unwrap()
    };
    let mut table_asset = get("pseudoregalia/Content/Data/DataTables/DT_OutfitData");
    let mut table_names = table_asset.get_name_map();
    let table = &mut unreal_asset::cast!(
        Export,
        DataTableExport,
        &mut table_asset.asset_data.exports[0]
    )
    .unwrap()
    .table
    .data;
    let mut outfits = vec![];
    let mut modfiles = repak::PakWriter::new(
        std::fs::File::create("~mods/costumes_p.pak").unwrap(),
        repak::Version::V11,
        "../../../".to_string(),
        None,
    );
    for (costume_name, pak, mut file) in std::fs::read_dir("outfits")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter_map(|entry| {
            (entry.extension() == Some(std::ffi::OsStr::new("pak"))).then(|| {
                (
                    entry.file_stem().unwrap().to_string_lossy().to_string(),
                    repak::PakReader::new_any(&mut std::fs::File::open(&entry).unwrap()).unwrap(),
                    std::fs::File::open(&entry).unwrap(),
                )
            })
        })
    {
        let mut table_names = table_names.get_mut();
        let path = "pseudoregalia/Content/Meshes/Characters/".to_string() + &costume_name;
        let mount = pak.mount_point().trim_start_matches("../../../");
        for asset in pak.files() {
            modfiles
                .write_file(
                    &(mount.to_string() + &asset),
                    &mut pak.get(&asset, &mut file).unwrap(),
                )
                .unwrap();
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
    table_asset
        .write_data(&mut table.0, Some(&mut table.1))
        .unwrap();
    modfiles
        .write_file(
            "pseudoregalia/Content/Data/DataTables/DT_OutfitData.uasset",
            table.0.into_inner(),
        )
        .unwrap();
    modfiles
        .write_file(
            "pseudoregalia/Content/Data/DataTables/DT_OutfitData.uexp",
            table.1.into_inner(),
        )
        .unwrap();
    modfiles.write_index().unwrap();

    let Some(saves) = std::env::var_os("USERPROFILE")
        .filter(|home| !home.is_empty())
        .map(std::path::PathBuf::from)
        .map(|path| path.join("AppData/Local"))
        .map(|path| path.join("pseudoregalia/Saved/SaveGames"))
        .map(|path| {
            path.read_dir()
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter_map(
                    |entry| match entry.extension() == Some(std::ffi::OsStr::new("sav")) {
                        true => {
                            match gvas::GvasFile::read(&mut std::fs::File::open(&entry).unwrap()) {
                                Ok(save) => Some((save, entry)),
                                Err(e) => {
                                    eprintln!("writing to {entry:?}: {e}");
                                    None
                                }
                            }
                        }
                        false => None,
                    },
                )
        })
    else {
        return;
    };
    for (mut save, path) in saves {
        let Some(unlocked) = save
            .properties
            .get_mut("unlockedOutfits")
            .and_then(gvas::properties::Property::get_array_mut)
        else {
            continue;
        };
        let old: Vec<_> = unlocked
            .properties
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(i, outfit)| {
                outfit
                    .get_name()
                    .is_some_and(|name| {
                        ["base", "greaves", "glove", "pants", "pro"]
                            .contains(&name.value.to_ascii_lowercase().as_str())
                    })
                    .then_some(i)
            })
            .collect();
        for i in old {
            unlocked.properties.remove(i);
        }
        unlocked.properties.append(&mut outfits);
        save.write(&mut std::fs::File::create(&path).unwrap())
            .unwrap();
        println!("{:?} written", path.file_name().unwrap_or_default());
    }
    println!("finished! you can now launch the game");
    println!("press enter to exit :)");
    std::io::stdin().read_line(&mut String::new()).unwrap();
}
