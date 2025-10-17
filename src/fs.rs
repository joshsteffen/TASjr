use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use zip::ZipArchive;

pub struct Fs {
    roots: Vec<PathBuf>,
    containing_pk3_map: HashMap<String, Pk3Entry>,
}

struct Pk3Entry {
    pk3_path: PathBuf,
    entry_path: String,
}

impl Fs {
    pub fn new<P: AsRef<Path>>(roots: &[P]) -> Result<Self, Box<dyn Error>> {
        let roots: Vec<_> = roots.iter().map(|root| root.as_ref().to_owned()).collect();
        let mut containing_pk3_map = HashMap::new();
        let mut priority_map = HashMap::new();

        for (priority, root) in roots.iter().enumerate() {
            let pk3_paths = fs::read_dir(root)?
                .filter_map(|res| res.ok())
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "pk3"));

            for pk3_path in pk3_paths {
                let mut pk3 = ZipArchive::new(File::open(&pk3_path)?)?;

                for i in 0..pk3.len() {
                    let file = pk3.by_index(i)?;
                    let key = file.name().to_lowercase();

                    if priority_map
                        .get(&key)
                        .is_some_and(|&old_priority| old_priority < priority)
                    {
                        continue;
                    }

                    let should_insert = containing_pk3_map
                        .get(&key)
                        .is_none_or(|entry: &Pk3Entry| pk3_path > entry.pk3_path);

                    if should_insert {
                        containing_pk3_map.insert(
                            key.clone(),
                            Pk3Entry {
                                pk3_path: pk3_path.to_path_buf(),
                                entry_path: file.name().to_string(),
                            },
                        );
                        priority_map.insert(key, priority);
                    }
                }
            }
        }

        Ok(Self {
            roots,
            containing_pk3_map,
        })
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<Cursor<Vec<u8>>, Box<dyn Error>> {
        let path = path.as_ref();

        for root in &self.roots {
            if let Ok(data) = fs::read(root.join(path)) {
                return Ok(Cursor::new(data));
            }
        }

        if let Some(entry) = self
            .containing_pk3_map
            .get(&path.to_str().unwrap().to_lowercase())
        {
            let mut data = vec![];
            let mut pk3 = ZipArchive::new(File::open(&entry.pk3_path)?)?;
            pk3.by_name(&entry.entry_path)?.read_to_end(&mut data)?;
            return Ok(Cursor::new(data));
        }

        Err(format!("failed to load {path:?}"))?
    }

    pub fn read<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(self.open(path)?.into_inner())
    }
}
