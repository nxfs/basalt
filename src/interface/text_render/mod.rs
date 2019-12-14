#![allow(warnings)]

use allsorts::binary::read::ReadScope;
use allsorts::font_data_impl::read_cmap_subtable;
use allsorts::gpos::{gpos_apply,Info,Placement};
use allsorts::gsub::{gsub_apply_default,RawGlyph,GlyphOrigin};
use allsorts::tables::cmap::Cmap;
use allsorts::tables::{MaxpTable,OpenTypeFile,OpenTypeFont};
use allsorts::layout::{new_layout_cache,GDEFTable,LayoutTable,GPOS,GSUB};
use allsorts::tag;
use allsorts::tables::HmtxTable;
use allsorts::tables::HheaTable;
use allsorts::tables::glyf::GlyfTable;
use allsorts::tables::loca::LocaTable;
use allsorts::tables::HeadTable;
use allsorts::tables::loca::LocaOffsets;
use allsorts::tables::glyf::{self,GlyfRecord};

#[derive(Debug)]
pub struct BasaltGlyph {
	pub x: i32,
	pub y: i32,
	pub bounds_min: [i16; 2],
	pub bounds_max: [i16; 2],
	pub geometry: Vec<Geometry>,
}

pub struct GlyphBitmap {
	width: u32,
	height: u32,
	data: Vec<u8>,
}

impl GlyphBitmap {
	fn draw_line(&mut self, glyph: &BasaltGlyph, points: &[f32]) {
		let diff_x = points[2] - points[0];
		let diff_y = points[3] - points[1];
		let steps = (diff_x.powi(2) + diff_y.powi(2)).sqrt().ceil() as usize;
		
		for s in 0..=steps {
			let x = (points[0] + ((diff_x / steps as f32) * s as f32)).floor() - glyph.bounds_min[0] as f32;
			let y = (points[1] + ((diff_y / steps as f32) * s as f32)).ceil() - glyph.bounds_min[1] as f32;
			
			if let Some(v) = self.data.get_mut(((self.width as f32 * (self.height as f32 - y)) + x).trunc() as usize) {
				*v = 255;
			}
		}
	}
	
	fn draw_curve(&mut self, glyph: &BasaltGlyph, points: &[f32]) {
		let steps = 10_usize;
		let mut last: Box<[f32; 2]> = Box::new([points[0], points[1]]);
		
		for s in 0..=steps {
			let t = s as f32 / steps as f32;
			let next = Box::new(
				lerp(
					t,
					&lerp(t, &points[0..2], &points[2..4]),
					&lerp(t, &points[2..4], &points[4..6])
				)
			);
			
			self.draw_line(glyph, [*last, *next].concat().as_slice());
			last = next;
		}
	}
}

impl BasaltGlyph {
	pub fn bitmap(&self) -> Result<GlyphBitmap, String> {
		let width = (self.bounds_max[0] - self.bounds_min[0]) as u32;
		let height = (self.bounds_max[1] - self.bounds_min[1]) as u32;
		let mut data = Vec::with_capacity((width * height) as usize);
		data.resize((width * height) as usize, 0);
		
		let mut bitmap = GlyphBitmap {
			width,
			height,
			data
		};
		
		self.geometry.iter().for_each(|g| match g {
			&Geometry::Line(p) => bitmap.draw_line(self, &p),
			&Geometry::Curve(p) => bitmap.draw_curve(self, &p)
		});
		
		Ok(bitmap)
	}
}

#[derive(Debug)]
pub enum Geometry {
	Line([f32; 4]),
	Curve([f32; 6]),
}

#[inline]
fn lerp(t: f32, p1: &[f32], p2: &[f32]) -> [f32; 2] {
	[
		p1[0] + ((p2[0] - p1[0]) * t),
		p1[1] + ((p2[1] - p1[1]) * t)
	]
}

pub fn shape_text<T: AsRef<str>>(text: T, script: u32, lang: u32) -> Result<Vec<BasaltGlyph>, String> {
	let file = ReadScope::new(include_bytes!("../ABeeZee-Regular.ttf")).read::<OpenTypeFile>().unwrap();
	let scope = file.scope;
	let otf = match file.font { OpenTypeFont::Single(v) => v, _ => panic!() };	
	let cmap = otf.read_table(&scope, tag::CMAP).unwrap().map(|v| v.read::<Cmap>().unwrap()).unwrap();
	let cmap_subtable = read_cmap_subtable(&cmap).unwrap().unwrap();
	let maxp = otf.read_table(&scope, tag::MAXP).unwrap().map(|v| v.read::<MaxpTable>().unwrap()).unwrap();
	/*let gsub_table = otf.find_table_record(tag::GSUB).unwrap()
		.read_table(&scope).unwrap().read::<LayoutTable<GSUB>>().unwrap();*/
	let opt_gdef_table = otf.find_table_record(tag::GDEF).map(|gdef_record|
		gdef_record.read_table(&scope).unwrap().read::<GDEFTable>().unwrap());
	let opt_gpos_table = otf.find_table_record(tag::GPOS).map(|gpos_record|
		gpos_record.read_table(&scope).unwrap().read::<LayoutTable<GPOS>>().unwrap());
	// let gsub_cache = new_layout_cache(gsub_table);
	let hhea = otf.find_table_record(tag::HHEA).unwrap().read_table(&scope).unwrap().read::<HheaTable>().unwrap();
	let hmtx = otf.find_table_record(tag::HMTX).unwrap().read_table(&scope).unwrap().read_dep::<HmtxTable>((maxp.num_glyphs as usize, hhea.num_h_metrics as usize)).unwrap();
	let head = otf.find_table_record(tag::HEAD).unwrap().read_table(&scope).unwrap().read::<HeadTable>().unwrap();
	let loca = otf.find_table_record(tag::LOCA).unwrap().read_table(&scope).unwrap().read_dep::<LocaTable>((maxp.num_glyphs as usize, head.index_to_loc_format)).unwrap();
	let mut glyf = otf.find_table_record(tag::GLYF).unwrap().read_table(&scope).unwrap().read_dep::<GlyfTable>(&loca).unwrap();
	
	let map_char = |c| {
		RawGlyph {
			unicodes: vec![c],
			glyph_index: cmap_subtable.map_glyph(c as u32).unwrap(),
			liga_component_pos: 0,
			glyph_origin: GlyphOrigin::Char(c),
			small_caps: false,
			multi_subst_dup: false,
			is_vert_alt: false,
			fake_bold: false,
			fake_italic: false,
			extra_data: (),
		}
	};
	
	let mut glyphs: Vec<_> = text.as_ref().chars().map(|c| map_char(c)).collect();
	
	/*gsub_apply_default(
		&|| vec![map_char('\u{25cc}')],
		&gsub_cache,
		opt_gdef_table.as_ref(),
		script,
		lang,
		false,
		maxp.num_glyphs,
		&mut glyphs
	).unwrap();*/
	
	let mut infos = Info::init_from_glyphs(opt_gdef_table.as_ref(), glyphs).unwrap();
	
	if let Some(gpos_table) = opt_gpos_table {
		let gpos_cache = new_layout_cache(gpos_table);
		let kerning = true;
	
		gpos_apply(
			&gpos_cache,
			opt_gdef_table.as_ref(),
			kerning,
			script,
			lang,
			&mut infos
		).unwrap();
	}
	
	let mut x: i32 = 0;
	let y: i32 = 0;
	let mut out = Vec::new();
	
	for (i, info) in infos.into_iter().enumerate() {
		let ha = match i {
			0 => 0,
			_ => hmtx.horizontal_advance(info.glyph.glyph_index.unwrap(), hhea.num_h_metrics).unwrap()
		} as i32;
		
		let (gx, gy) = match info.placement {
			Placement::Distance(dx, dy) => (
				((x + ha + dx) - 1500) * i as i32,
				y + dy
			),
			Placement::Anchor(_, _) | Placement::None => (
				((x + ha) - 1500) * i as i32,
				y
			)
		};
		
		let glyf_record = glyf.records.get_mut(info.glyph.glyph_index.unwrap() as usize).unwrap();
		
		match match &glyf_record {
			&GlyfRecord::Present(ref s) => Some(GlyfRecord::Parsed(s.read::<glyf::Glyph>().unwrap())),
			_ => None
		} {
			Some(new_record) => *glyf_record = new_record,
			None => ()
		}
		
		let (bounds_min, bounds_max, geometry) = match &glyf_record {
			&GlyfRecord::Parsed(ref glfy_glyph) => {
				let bounds_min = [glfy_glyph.bounding_box.x_min, glfy_glyph.bounding_box.y_min];
				let bounds_max = [glfy_glyph.bounding_box.x_max, glfy_glyph.bounding_box.y_max];
				let geometry = match &glfy_glyph.data {
					 &glyf::GlyphData::Simple(ref simple) => {
						let mut geometry = Vec::new();
						let mut contour = Vec::new();
						
						for i in 0..simple.coordinates.len() {
							contour.push((
								i,
								simple.coordinates[i].0 as f32,
								simple.coordinates[i].1 as f32
							));
						
							if simple.end_pts_of_contours.contains(&(i as u16)) {
								for j in 0..contour.len() {
									if !simple.flags[contour[j].0].is_on_curve() {
										let p_i = if j == 0 {
											contour.len() - 1
										} else {
											j - 1
										}; let n_i = if j == contour.len() - 1 {
											0
										} else {
											j + 1
										};
										
										let a = if simple.flags[contour[p_i].0].is_on_curve() {
											(contour[p_i].1, contour[p_i].2)
										} else {
											(
												(contour[p_i].1 + contour[j].1) / 2.0,
												(contour[p_i].2 + contour[j].2) / 2.0
											)
										};
										
										let c = if simple.flags[contour[n_i].0].is_on_curve() {
											(contour[n_i].1, contour[n_i].2)
										} else {
											(
												(contour[n_i].1 + contour[j].1) / 2.0,
												(contour[n_i].2 + contour[j].2) / 2.0
											)
										};
										
										let b = (contour[j].1, contour[j].2);
										geometry.push(Geometry::Curve([a.0, a.1, b.0, b.1, c.0, c.1]));
									} else {
										let n_i = if j == contour.len() - 1 {
											0
										} else {
											j + 1
										};
										
										if simple.flags[contour[n_i].0].is_on_curve() {
											geometry.push(Geometry::Line([contour[j].1, contour[j].2, contour[n_i].1, contour[n_i].2]));
										}
									}
								}
								
								contour.clear();
							}
						}
						
						geometry
					},
					glyf::GlyphData::Composite { .. } => panic!("Composite glyphs are not supported yet!")
				};
				
				(bounds_min, bounds_max, geometry)
			},
			&GlyfRecord::Present(_) => panic!("Glyph should already be parsed!"),
			&GlyfRecord::Empty => ([0, 0], [0, 0], Vec::new())
		};
		
		out.push(BasaltGlyph {
			x: gx,
			y: gy,
			geometry,
			bounds_min,
			bounds_max,
		});
		
		x += ha;
	}
	
	Ok(out)
}

#[test]
fn render_text() {
	use crate::interface::bin::{self,BinStyle,PositionTy};
	
	let glyphs = shape_text("Hello World!", tag::from_string("DFLT").unwrap(), tag::from_string("dflt").unwrap()).unwrap();
	let mut bitmap = glyphs[1].bitmap().unwrap();
	
	let basalt = crate::Basalt::new(
		crate::Options::default()
			.ignore_dpi(true)
			.window_size(bitmap.width + 20, bitmap.height + 20)
			.title("Basalt")
	).unwrap();
	basalt.spawn_app_loop();
	
	let image = crate::atlas::Image::new(
		crate::atlas::ImageType::SMono,
		crate::atlas::ImageDims {
			w: bitmap.width, 
			h: bitmap.height,
		},
		crate::atlas::ImageData::D8(bitmap.data)
	).unwrap();
	
	let coords = basalt.atlas_ref().load_image(crate::atlas::SubImageCacheID::None, image).unwrap();
	let background = basalt.interface_ref().new_bin();
	
	background.style_update(BinStyle {
		position_t: Some(PositionTy::FromWindow),
		pos_from_t: Some(10.0),
		pos_from_b: Some(10.0),
		pos_from_l: Some(10.0),
		pos_from_r: Some(10.0),
		back_image_atlas: Some(coords),
		.. background.style_copy()
	});
	
	basalt.wait_for_exit().unwrap();
}