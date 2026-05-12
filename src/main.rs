use arc_diff::compression;
use compression::iso;

#[allow(unused_imports)]
use compression::bytes;
use cstr::cstr;
use qmetaobject::*;
use std::path::PathBuf;

#[derive(QObject, Default)]
struct DiffBackend {
    base: qt_base_class!(trait QObject),
    compare_iso_with_folder: qt_method!(
        fn compare_iso_with_folder(&self, iso_path: QString, folder_path: QString) -> QString {
            let iso_path = PathBuf::from(iso_path.to_string());
            let folder_path = PathBuf::from(folder_path.to_string());
            match iso::diff_iso_files_against_folder(&iso_path, &folder_path) {
                Ok(result) => result.into(),
                Err(err) => format!("Error: {err}").into(),
            }
        }
    ),
}

fn main() {
    qml_register_type::<DiffBackend>(cstr!("ArcDiff"), 1, 0, cstr!("DiffBackend"));

    let qml = include_str!("qml/main.qml");
    let mut engine = QmlEngine::new();
    engine.load_data(qml.into());
    engine.exec();
}
