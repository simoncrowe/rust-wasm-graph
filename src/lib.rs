use byteorder::{ByteOrder, LittleEndian};

use js_sys::Uint8Array;
use log::{debug, Level};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use web_sys::window;
extern crate console_error_panic_hook;
use std::panic;

mod geometry;

const NODE_DISPLAY_SQUARE_WIDTH: f32 = 8.0;
const DISPLAY_PAN_RATE: f32 = 1.0;

#[wasm_bindgen]
pub fn init_logging() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Debug).expect("Console logging failed to initialise");
}

#[wasm_bindgen]
pub fn get_memory() -> JsValue {
    wasm_bindgen::memory()
}

#[wasm_bindgen]
pub struct GraphFacade {
    graph: GraphDisplay,
}

#[wasm_bindgen]
impl GraphFacade {
    pub fn new(
        node_count: usize,
        spawn_scale: f32,
        display_width: f32,
        display_height: f32,
        display_scale: f32,
    ) -> GraphFacade {
        let layout = GraphLayout::new(node_count, spawn_scale);
        let display = GraphDisplay::new(layout, display_width, display_height, display_scale);
        GraphFacade { graph: display }
    }

    pub fn load_edges(&mut self, chunk_array: Uint8Array) {
        self.graph.layout.load_edges(chunk_array);
    }

    pub fn update_display_size(&mut self, display_width: f32, display_height: f32) {
        self.graph
            .update_display_size(display_width, display_height);
    }

    pub fn update_vertices(&mut self) {
        self.graph.update_vertices();
    }

    pub fn get_vertices_ptr(&self) -> *const f32 {
        self.graph.get_vertices_ptr()
    }

    pub fn translate_offset_by_pixels(&mut self, x: f32, y: f32) {
        self.graph.translate_offset_by_pixels(x, y);
    }
}

pub struct GraphLayout {
    node_targets: Vec<Vec<usize>>,
    node_sources: Vec<Vec<usize>>,
    node_locations: Vec<Vec<f32>>,
    loading_node_index: usize,
}

impl GraphLayout {
    pub fn new(node_count: usize, spawn_scale: f32) -> GraphLayout {
        let node_targets = (0..node_count).map(|_| Vec::new()).collect();
        let node_sources = (0..node_count).map(|_| Vec::new()).collect();
        let node_locations: Vec<Vec<f32>> = (0..node_count)
            .map(|_| geometry::random_location(spawn_scale, spawn_scale))
            .collect();
        GraphLayout {
            node_targets,
            node_sources,
            node_locations,
            loading_node_index: 0,
        }
    }
    pub fn load_edges(&mut self, chunk_array: Uint8Array) {
        debug!("Started loading edges");
        let chunk_buffer = chunk_array.to_vec();
        let mut numbers = vec![0; chunk_buffer.len() / 2];
        LittleEndian::read_u16_into(&chunk_buffer, &mut numbers);
        debug!("Getting targets for node {}...", self.loading_node_index);
        for &num in numbers.iter() {
            // The MAX acts as a delimiter
            if num == u16::MAX {
                self.loading_node_index += 1;
            } else {
                let target_index = num as usize;
                self.node_targets
                    .get_mut(self.loading_node_index)
                    .unwrap()
                    .push(target_index);
                self.node_sources
                    .get_mut(target_index)
                    .unwrap()
                    .push(self.loading_node_index);
            }
        }
    }
}

pub struct GraphDisplay {
    layout: GraphLayout,
    display_width: f32,
    display_height: f32,
    display_scale: f32,
    display_offset: Vec<f32>,
    clipspace_vertices: Vec<f32>,
    clipspace_square_offset: f32,
}

impl GraphDisplay {
    pub fn new(
        layout: GraphLayout,
        display_width: f32,
        display_height: f32,
        display_scale: f32,
    ) -> GraphDisplay {
        let aspect_ratio = display_width / display_height;
        let display_offset = layout.node_locations[0].clone();
        let clipspace_square_offset = NODE_DISPLAY_SQUARE_WIDTH / 2.0 / display_height;
        let clipspace_vertices = layout
            .node_locations
            .iter()
            .map(|loc| {
                geometry::layout_to_display(&loc, &display_offset, &display_scale, &aspect_ratio)
            })
            .map(|loc| geometry::square_vertices(&loc, &aspect_ratio, &clipspace_square_offset))
            .flatten()
            .collect();
        GraphDisplay {
            layout,
            display_width,
            display_height,
            display_scale,
            display_offset,
            clipspace_vertices,
            clipspace_square_offset,
        }
    }

    pub fn translate_offset_by_pixels(&mut self, x: f32, y: f32) {
        let pan_rate = DISPLAY_PAN_RATE / self.display_height;
        self.display_offset[0] -= (x * pan_rate) / self.display_aspect_ratio();
        self.display_offset[1] += y * pan_rate
    }

    pub fn update_display_size(&mut self, display_width: f32, display_height: f32) {
        self.display_width = display_width;
        self.display_height = display_height;
    }

    pub fn update_vertices(&mut self) {
        let aspect_ratio = self.display_aspect_ratio();
        self.clipspace_vertices = self
            .layout
            .node_locations
            .iter()
            .map(|loc| {
                geometry::layout_to_display(
                    &loc,
                    &self.display_offset,
                    &self.display_scale,
                    &aspect_ratio,
                )
            })
            .map(|loc| {
                geometry::square_vertices(&loc, &aspect_ratio, &self.clipspace_square_offset)
            })
            .flatten()
            .collect();
    }

    pub fn get_vertices_ptr(&self) -> *const f32 {
        self.clipspace_vertices.as_ptr()
    }

    pub fn node_ids_to_render(&mut self, rect: geometry::Rect) -> Vec<usize> {
        let perf = window().unwrap().performance().unwrap();
        let start = perf.now();

        let mut indices = rustc_hash::FxHashSet::default();

        for (idx, loc) in self.layout.node_locations.iter().enumerate() {
            if rect.contains(loc) {
                indices.insert(idx);
                for source_idx in self.layout.node_sources.get(idx).unwrap().into_iter() {
                    indices.insert(*source_idx);
                }
                for target_idx in self.layout.node_targets.get(idx).unwrap().into_iter() {
                    indices.insert(*target_idx);
                }
            }
        }
        let result = indices.into_iter().collect::<Vec<usize>>();

        let elapsed = perf.now() - start;
        debug!("node_ids_to_render took {} ms", elapsed);

        result
    }
}

impl GraphDisplay {
    fn display_aspect_ratio(&self) -> f32 {
        self.display_width / self.display_height
    }
}
