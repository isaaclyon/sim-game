//! Fire (ADR 0053): type conversion only, like the rest of this crate.

use sim::Command;
use wasm_bindgen::prelude::*;

use crate::Sim;

/// Fire figures for the top bar; see `sim::FireStats`.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct FireView {
    pub enabled: bool,
    /// No fire starts below this many residents.
    pub threshold: u32,
    pub stations: u32,
    pub trucks: u32,
    pub free_trucks: u32,
    pub burning: u32,
    /// Burning with no truck on the way.
    pub unattended: u32,
    pub covered: u32,
    pub saved: u32,
    pub destroyed: u32,
}

#[wasm_bindgen]
impl Sim {
    /// Places a fire station beside the nearest road, facing it. False if refused.
    pub fn place_station(&mut self, x: f32, y: f32) -> bool {
        self.apply(Command::PlaceStation { x, y })
    }

    /// Turns this city's fires on or off; false if it already was.
    pub fn set_fires(&mut self, on: bool) -> bool {
        self.apply(Command::SetFires { on })
    }

    /// `[ok, x0, y0, …]`: whether a station asked for here would be placed, then its closed
    /// footprint; empty where no road is in reach. See `World::write_station_preview`.
    pub fn station_preview(&mut self, x: f32, y: f32) -> Vec<f32> {
        let ok = self.world.write_station_preview(x, y, &mut self.buf);
        let mut out = Vec::with_capacity(self.buf.len() + 1);
        if !self.buf.is_empty() {
            out.push(f32::from(u8::from(ok)));
            out.extend_from_slice(&self.buf);
        }
        out
    }

    pub fn fire_version(&self) -> u32 {
        self.world.fire_version()
    }

    /// `[x, y, half_size, front_x, front_y]` per station; see `World::write_fire_stations`.
    pub fn fire_stations(&mut self) -> Vec<f32> {
        self.world.write_fire_stations(&mut self.buf);
        self.buf.clone()
    }

    /// `[x, y, height, truck]` per burning building; see `World::write_fires`.
    pub fn fires(&mut self) -> Vec<f32> {
        self.world.write_fires(&mut self.buf);
        self.buf.clone()
    }

    /// `[n, x0, y0, …]` per lot holding ruins; see `World::write_ruins`.
    pub fn ruins(&mut self) -> Vec<f32> {
        self.world.write_ruins(&mut self.buf);
        self.buf.clone()
    }

    /// Covered road segments `[ax, ay, bx, by]`, then covered building centres `[x, y]`, after
    /// a leading count of road floats; see `World::write_fire_cover`.
    pub fn fire_cover(&mut self) -> Vec<f32> {
        let mut buildings = Vec::new();
        self.world.write_fire_cover(&mut self.buf, &mut buildings);
        let mut out = Vec::with_capacity(1 + self.buf.len() + buildings.len());
        out.push(self.buf.len() as f32);
        out.extend_from_slice(&self.buf);
        out.extend_from_slice(&buildings);
        out
    }

    pub fn fire_stats(&self) -> FireView {
        let f = self.world.fire_stats();
        FireView {
            enabled: f.enabled,
            threshold: f.threshold,
            stations: f.stations,
            trucks: f.trucks,
            free_trucks: f.free_trucks,
            burning: f.burning,
            unattended: f.unattended,
            covered: f.covered,
            saved: f.saved,
            destroyed: f.destroyed,
        }
    }
}
