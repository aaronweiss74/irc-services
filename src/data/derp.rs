#![cfg(feature = "derp")]
use std::borrow::ToOwned;
use std::error::Error;
use std::old_io::{File, FilePermission, InvalidInput, IoError, IoResult};
use std::old_io::fs::mkdir_recursive;
use rustc_serialize::json::{decode, encode};

#[derive(RustcEncodable, RustcDecodable, Debug, PartialEq)]
pub struct DerpCounter {
    derps: usize,
}

impl DerpCounter {
    pub fn load() -> IoResult<DerpCounter> {
        let path = "data/derp.json".to_owned();
        let file = File::open(&Path::new(&path));
        if let Ok(mut file) = file {
            let data = try!(file.read_to_string());
            decode(&data).map_err(|e| IoError {
                kind: InvalidInput,
                desc: "Failed to decode derp data.",
                detail: Some(e.description().to_owned()),
            })
        } else {
            Ok(DerpCounter { derps: 0 })
        }
    }

    pub fn save(&self) -> IoResult<()> {
        let mut path = "data/".to_owned();
        try!(mkdir_recursive(&Path::new(&path), FilePermission::all()));
        path.push_str("derp.json");
        let mut f = File::create(&Path::new(&path));
        f.write_str(&try!(encode(self).map_err(|e| IoError {
            kind: InvalidInput,
            desc: "Failed to encode derp data.",
            detail: Some(e.description().to_owned()),
        }))[..])
    }

    pub fn increment(&mut self) {
        self.derps += 1
    }

    pub fn derps(&self) -> usize {
        self.derps
    }
}

#[cfg(test)]
mod test {
    use super::DerpCounter;
    
    #[test]
    fn save() {
        let derp = DerpCounter::load();
        assert!(derp.is_ok());
        let mut derp = derp.unwrap();
        derp.increment();
        assert!(derp.save().is_ok());
    }
}
