//! Collect definitions for `Residue` and `SheetConf` objects
//! into a `DataBase` which can be read from or saved to disk.

use coord::{Coord, Translate};
use describe::{describe_list_short, describe_list, Describe};
use iterator::AtomIterItem;
use surface;
use system::{Component, Residue};
use volume;

use serde_json;
use std::ffi::OsStr;
use std::fmt::Write;
use std::convert::From;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::result;

#[derive(Copy, Clone, Debug)]
pub enum DataBaseError {
    BadPath,
}

#[macro_export]
/// Macro to wrap every object constructor into an enum with desired traits.
///
/// The traits are those important for the creation and display of system components.
/// The enum is used to hold created objects of different types in one container,
/// sharing one interface.
///
/// Implements `Describe`, `Component` and `Translate` for the enum.
///
/// Also sets up some getter functions directly to the object data and
/// the `with_pbc` method to move residue coordinates within the box.
///
/// # Requires
/// Wrapped objects have to implement the above traits and `Clone`, `Debug`,
/// `Deserialize` and `Serialize` (the last two from `serde`).
///
/// # Examples
/// Create two objects and let the macro create the wrapper and implement the traits for it.
///
/// ```
/// # #[macro_use] extern crate grafen;
/// # extern crate serde_json;
/// # #[macro_use] extern crate serde_derive;
/// # use grafen::coord::{Coord, Translate};
/// # use grafen::describe::Describe;
/// # use grafen::iterator::{AtomIterator, AtomIterItem};
/// # use grafen::system::{Component, Residue};
/// #
/// #[derive(Clone, Debug, Deserialize, Serialize)]
/// pub struct StructOne {
///     origin: Coord,
///     residue: Option<Residue>,
///     coords: Vec<Coord>
/// }
///
/// #[derive(Clone, Debug, Deserialize, Serialize)]
/// pub struct StructTwo {
///     origin: Coord,
///     residue: Option<Residue>,
///     coords: Vec<Coord>
/// }
///
/// // Not shown: implement required traits
/// # impl StructOne { fn calc_box_size(&self) -> Coord { Coord::default() } }
/// # impl StructTwo { fn calc_box_size(&self) -> Coord { Coord::default() } }
/// # impl Describe for StructOne {
/// #     fn describe(&self) -> String { "StructOne".to_string() }
/// #     fn describe_short(&self) -> String { self.describe() }
/// # }
/// # impl Describe for StructTwo {
/// #     fn describe(&self) -> String { "StructTwo".to_string() }
/// #     fn describe_short(&self) -> String { self.describe() }
/// # }
/// # impl_component![StructOne, StructTwo];
/// # impl_translate![StructOne, StructTwo];
///
/// // Construct the wrapping enum container
/// create_entry_wrapper![
///     Wrapper, // enum identifier
///     (StructOne => One), // Wrapper::One(StructOne)
///     (StructTwo => Two)  // Wrapper::Two(StructTwo)
/// ];
///
/// #
/// # fn main() {
/// let objects = vec![
///     Wrapper::One(StructOne {
///         origin: Coord::default(),
///         residue: None,
///         coords: vec![]
///     }),
///     Wrapper::Two(StructTwo {
///         origin: Coord::default(),
///         residue: None,
///         coords: vec![]
///     })
/// ];
///
/// assert_eq!("StructOne", &objects[0].describe());
/// assert_eq!(None, objects[0].iter_atoms().next());
///
/// assert_eq!("StructTwo", &objects[1].describe());
/// assert_eq!(None, objects[1].iter_atoms().next());
/// # }
/// ```
macro_rules! create_entry_wrapper {
    (
        $name:ident, // enum Identifier
        $( ($class:path => $entry:ident) ),+ // Identifier::Entry(Class)
    ) => {
        #[derive(Clone, Debug, Deserialize, Serialize)]
        /// Wrapper for accessing a shared interface from different components constructors.
        pub enum $name {
            $(
                $entry($class),
            )*
        }

        impl<'a> $name {
            /// Get a reference to the coordinates of the component.
            pub fn get_coords(&'a self) -> &Vec<Coord> {
                match *self {
                    $(
                        $name::$entry(ref object) => &object.coords,
                    )*
                }
            }

            /// Get a mutable reference to the coordinates of the component.
            pub fn get_coords_mut(&'a mut self) -> &mut Vec<Coord> {
                match *self {
                    $(
                        $name::$entry(ref mut object) => &mut object.coords,
                    )*
                }
            }

            /// Get a reference to the component's optional `Residue`.
            pub fn get_residue(&'a self) -> &'a Option<Residue> {
                match *self {
                    $(
                        $name::$entry(ref object) => &object.residue,
                    )*
                }
            }

            /// Apply periodic boundary conditions to each residue coordinate
            /// to move them inside the component box.
            pub fn with_pbc(mut self) -> Self {
                let box_size = self.box_size();

                self.get_coords_mut()
                    .iter_mut()
                    .for_each(|c| *c = c.with_pbc(box_size));

                self
            }
        }

        impl Describe for $name {
            fn describe(&self) -> String {
                match *self {
                    $(
                        $name::$entry(ref object) => object.describe(),
                    )*
                }
            }

            fn describe_short(&self) -> String {
                match *self {
                    $(
                        $name::$entry(ref object) => object.describe_short(),
                    )*
                }
            }
        }

        impl<'a> Component<'a> for $name {
            fn box_size(&self) -> Coord {
                match *self {
                    $(
                        $name::$entry(ref object) => object.box_size(),
                    )*
                }
            }

            fn iter_atoms(&'a self) -> AtomIterItem {
                match *self {
                    $(
                        $name::$entry(ref object) => object.iter_atoms(),
                    )*
                }
            }

            fn num_atoms(&self) -> u64 {
                match *self {
                    $(
                        $name::$entry(ref object) => object.num_atoms(),
                    )*
                }
            }
        }

        impl Translate for $name {
            fn translate(self, coord: Coord) -> Self {
                match self {
                    $(
                        $name::$entry(object) => $name::$entry(object.translate(coord)),
                    )*
                }
            }

            fn translate_in_place(&mut self, coord: Coord) {
                match *self {
                    $(
                        $name::$entry(ref mut object)
                            => { object.translate_in_place(coord); }
                    )*
                }
            }
        }

        $(
            impl From<$class> for $name {
                fn from(object: $class) -> $name {
                    $name::$entry(object)
                }
            }
        )*
    }
}

// Our wrapper for object constructors is `ComponentEntry`. Use the macro to construct it.
create_entry_wrapper![
    ComponentEntry,
    (volume::Cuboid => VolumeCuboid),
    (volume::Cylinder => VolumeCylinder),
    (surface::Sheet => SurfaceSheet),
    (surface::Cylinder => SurfaceCylinder)
];

#[derive(Clone, Debug, Deserialize, Serialize)]
/// A collection of residues and substrate configurations
/// which can be saved to and read from disk.
pub struct DataBase {
    #[serde(skip)]
    /// A path to the `DataBase` location on the hard drive.
    pub path: Option<PathBuf>,

    #[serde(rename = "residue_definitions", default = "Vec::new")]
    /// Definitions of `Residue` objects.
    pub residue_defs: Vec<Residue>,

    #[serde(rename = "component_definitions", default = "Vec::new")]
    /// New component constructors.
    pub component_defs: Vec<ComponentEntry>,
}

impl DataBase {
    /// Construct an empty `DataBase`.
    pub fn new() -> DataBase {
        DataBase {
            path: None,
            residue_defs: vec![],
            component_defs: vec![],
        }
    }

    /// Get the database path enclosed in single quotes if it exists,
    /// otherwise the unenclosed string "None".
    pub fn get_path_pretty(&self) -> String {
        self.path.as_ref()
            .map(|path| path.to_string_lossy().to_owned())
            .map(|path| format!("'{}'", path))
            .unwrap_or("None".to_string())
    }

    /// Set a new path for the `DataBase`. The input path is asserted to
    /// be a file and the extension is set to 'json'.
    pub fn set_path<T>(&mut self, new_path: &T) -> Result<(), DataBaseError>
            where T: ?Sized + AsRef<OsStr> {
        let mut path = PathBuf::from(new_path);

        if path.file_stem().is_some() {
            path.set_extension("json");
            self.path = Some(path);
            Ok(())
        } else {
            Err(DataBaseError::BadPath)
        }
    }

    /// Parse a reader for a JSON formatted `DataBase`.
    ///
    /// This and the `to_writer` method are defined to enable a unit
    /// test which ensures that the behaviour for reading and writing
    /// a `DataBase` is consistent.
    fn from_reader<R: io::Read>(reader: R) -> Result<DataBase, io::Error> {
        serde_json::from_reader(reader).map_err(|e| io::Error::from(e))
    }

    /// Write a `DataBase` as a JSON formatted object to an input writer.
    fn to_writer<W: io::Write>(&self, writer: &mut W) -> result::Result<(), io::Error> {
        serde_json::to_writer_pretty(writer, self).map_err(|e| io::Error::from(e))
    }
}

impl Describe for DataBase {
    fn describe(&self) -> String {
        let mut description = String::new();
        const ERR: &'static str = "Could not construct a string";

        writeln!(description, "Database path: {}\n", self.get_path_pretty()).expect(ERR);
        writeln!(description, "{}", describe_list_short("Component definitions", &self.component_defs)).expect(ERR);
        writeln!(description, "{}", describe_list("Residue definitions", &self.residue_defs)).expect(ERR);

        description
    }

    fn describe_short(&self) -> String {
        self.describe()
    }
}

/// Read a `DataBase` from a JSON formatted file.
/// The owned path is set to the input path.
pub fn read_database(from_path: &str) -> Result<DataBase, io::Error> {
    let path = Path::new(from_path);
    let buffer = File::open(&path)?;

    let mut database = DataBase::from_reader(buffer)?;
    database.path = Some(PathBuf::from(&from_path));

    Ok(database)
}

/// Write a `DataBase` as a JSON formatted file.
/// The function writes to that owned by the object.
pub fn write_database(database: &DataBase) -> Result<(), io::Error> {
    if let Some(ref path) = database.path {
        let mut buffer = File::create(&path)?;
        database.to_writer(&mut buffer)?;

        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::Other,
        "No path was set when trying to write the database to disk")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use system::*;
    use surface::{LatticeType, Sheet};
    use volume::Cuboid;

    #[test]
    fn serialize_and_deserialize_residue_entry() {
        let base = Residue {
            code: "RES".to_string(),
            atoms: vec![
                Atom { code: "A1".to_string(), position: Coord::new(0.0, 1.0, 2.0) },
                Atom { code: "A2".to_string(), position: Coord::new(3.0, 4.0, 5.0) },
            ]
        };

        let serialized = serde_json::to_string(&base).unwrap();
        let deserialized: Residue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(base, deserialized);
    }

    #[test]
    fn database_by_default_sets_empty_vectors_if_not_available() {
        let database: DataBase = serde_json::from_str("{}").unwrap();
        assert!(database.residue_defs.is_empty());
        assert!(database.component_defs.is_empty());
    }

    #[test]
    fn read_and_write_database() {
        let base = Residue {
            code: "RES".to_string(),
            atoms: vec![
                Atom { code: "A1".to_string(), position: Coord::new(0.0, 1.0, 2.0) },
                Atom { code: "A2".to_string(), position: Coord::new(3.0, 4.0, 5.0) },
            ]
        };

        let database = DataBase {
            path: Some(PathBuf::from("This/will/be/removed")),
            residue_defs: vec![base.clone()],
            component_defs: vec![],
        };

        let mut serialized: Vec<u8> = Vec::new();
        database.to_writer(&mut serialized).unwrap();
        let deserialized = DataBase::from_reader(serialized.as_slice()).unwrap();

        assert_eq!(None, deserialized.path);
        assert_eq!(database.residue_defs, deserialized.residue_defs);
    }

    #[test]
    fn set_database_path() {
        let mut database = DataBase::new();
        assert!(database.set_path("test").is_ok());
        assert_eq!("test.json", database.path.unwrap().to_str().unwrap());
    }

    #[test]
    fn set_database_to_empty_path_is_error() {
        let mut database = DataBase::new();
        database.path = Some(PathBuf::from("unchanged.json"));
        assert!(database.set_path("").is_err());
        assert_eq!("unchanged.json", database.path.unwrap().to_str().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn get_database_path() {
        let mut database = DataBase::new();
        assert_eq!("None", &database.get_path_pretty());

        database.set_path("/a/file.json").unwrap();
        assert_eq!("'/a/file.json'", &database.get_path_pretty());
    }

    #[test]
    fn create_entry_macro_adds_from_method() {
        let cuboid = Cuboid::default();
        let component = ComponentEntry::from(cuboid.clone());

        match component {
            ComponentEntry::VolumeCuboid(object) => {
                assert_eq!(object.name, cuboid.name);
                assert_eq!(object.residue, cuboid.residue);
                assert_eq!(object.size, cuboid.size);
                assert_eq!(object.origin, cuboid.origin);
                assert_eq!(object.coords, cuboid.coords);
            },
            _ => panic!["Incorrect object was created"],
        }
    }

    #[test]
    fn component_entry_adds_with_pbc_method() {
        let sheet = Sheet {
            name: None,
            residue: None,
            lattice: LatticeType::Hexagonal { a: 0.1 },
            std_z: None,
            origin: Coord::ORIGO,
            length: 2.0,
            width: 1.0,
            coords: vec![
                Coord::new(0.5, 0.0, 0.0), // inside box
                Coord::new(1.5, 0.0, 0.0), // inside box
                Coord::new(2.5, 0.0, 0.0), // outside box by 0.5 along x
                Coord::new(0.0, 1.5, 0.0) // outside box by 0.5 along y
            ],
        };

        let component = ComponentEntry::from(sheet);

        let pbc_coords = vec![
            Coord::new(0.5, 0.0, 0.0), // unchanged
            Coord::new(1.5, 0.0, 0.0), // unchanged
            Coord::new(0.5, 0.0, 0.0), // moved to within box
            Coord::new(0.0, 0.5, 0.0) // moved to within box
        ];
        let pbc_component = component.with_pbc();

        assert_eq!(pbc_component.get_coords(), &pbc_coords);
    }
}
