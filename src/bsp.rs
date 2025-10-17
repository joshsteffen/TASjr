use std::{
    error::Error,
    io::{Read, Seek, SeekFrom},
    marker::PhantomData,
};

use binrw::{BinRead, Endian, binread, helpers::until_eof, io::TakeSeekExt};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug)]
#[binread]
#[br(little, magic = b"IBSP", assert(version == 46))]
pub struct Bsp {
    pub version: u32,
    pub entities: Lump<u8>,
    pub shaders: Lump<u8>,
    pub planes: Lump<u8>,
    pub nodes: Lump<u8>,
    pub leafs: Lump<u8>,
    pub leaf_surfaces: Lump<u8>,
    pub leaf_brushes: Lump<u8>,
    pub models: Lump<u8>,
    pub brushes: Lump<u8>,
    pub brush_sides: Lump<u8>,
    pub draw_verts: Lump<DrawVert>,
    pub draw_indexes: Lump<u32>,
    pub fogs: Lump<u8>,
    pub surfaces: Lump<Surface>,
    pub lightmaps: Lump<u8>,
    pub lightgrid: Lump<u8>,
    pub visibility: Lump<u8>,
}

#[derive(Debug)]
#[binread]
pub struct DrawVert {
    pub xyz: [f32; 3],
    pub st: [f32; 2],
    pub lightmap: [f32; 2],
    pub normal: [f32; 3],
    pub color: [u8; 4],
}

#[derive(Debug, PartialEq, Eq)]
#[binread]
#[br(repr = u32)]
pub enum MapSurfaceType {
    Bad,
    Planar,
    Patch,
    TriangleSoup,
    Flare,
}

#[derive(Debug)]
#[binread]
pub struct Surface {
    pub shader_num: u32,
    pub fog_num: i32,
    pub surface_type: MapSurfaceType,
    pub first_vert: u32,
    pub num_verts: u32,
    pub first_index: u32,
    pub num_indexes: u32,
    pub lightmap_num: i32,
    pub lightmap_x: u32,
    pub lightmap_y: u32,
    pub lightmap_width: u32,
    pub lightmap_height: u32,
    pub lightmap_origin: [f32; 3],
    pub lightmap_vecs: [[f32; 3]; 3],
    pub patch_width: u32,
    pub patch_height: u32,
}

#[derive(Debug)]
#[binread]
pub struct Lump<T> {
    pub fileofs: u32,
    pub filelen: u32,
    phantom: PhantomData<T>,
}

impl<T> Lump<T> {
    pub fn read<Arg, R: Read + Seek>(&self, mut reader: R) -> Result<Vec<T>>
    where
        T: for<'a> BinRead<Args<'a> = Arg>,
        Arg: Clone + Default,
    {
        reader.seek(SeekFrom::Start(self.fileofs.into()))?;
        let mut lump_reader = reader.take_seek(self.filelen.into());
        Ok(until_eof(
            &mut lump_reader,
            Endian::Little,
            Default::default(),
        )?)
    }
}
