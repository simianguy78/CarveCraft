//! Feeds & speeds calculator: recommends spindle speed, feed, plunge, pass depth and stepover
//! from the tool, the material and the machine, using chip load per tooth.

use crate::toolpath::{Tool, ToolKind};
use serde::{Deserialize, Serialize};

/// Materials the calculator knows (starting values, to be tuned on real cuts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stock {
    Softwood,
    Hardwood,
    Mdf,
    Plywood,
    Acrylic,
    Hdpe,
    Aluminium,
}

impl Stock {
    pub const ALL: [Stock; 7] = [Stock::Softwood, Stock::Hardwood, Stock::Mdf, Stock::Plywood, Stock::Acrylic, Stock::Hdpe, Stock::Aluminium];
    pub fn label(self) -> &'static str {
        match self {
            Stock::Softwood => "Softwood (pine, cedar)",
            Stock::Hardwood => "Hardwood (oak, maple)",
            Stock::Mdf => "MDF",
            Stock::Plywood => "Plywood",
            Stock::Acrylic => "Acrylic",
            Stock::Hdpe => "HDPE / soft plastic",
            Stock::Aluminium => "Aluminium",
        }
    }
    /// Best guess from a tool-library material name.
    pub fn from_name(name: &str) -> Stock {
        let n = name.to_lowercase();
        if n.contains("hard") || n.contains("oak") || n.contains("maple") || n.contains("walnut") {
            Stock::Hardwood
        } else if n.contains("mdf") {
            Stock::Mdf
        } else if n.contains("ply") {
            Stock::Plywood
        } else if n.contains("acryl") || n.contains("perspex") {
            Stock::Acrylic
        } else if n.contains("hdpe") || n.contains("plastic") {
            Stock::Hdpe
        } else if n.contains("alu") {
            Stock::Aluminium
        } else {
            Stock::Softwood
        }
    }
    /// Published chip-load ranges for 2-flute spiral router bits, in inches per tooth, at
    /// 1/8", 1/4" and 1/2" cutting diameter. Compiled from the charts router-bit makers
    /// commonly publish (Onsrud / Amana / Whiteside style tables); a specific tool's own
    /// spec sheet always takes priority.
    pub fn published_inches(self) -> [(f32, f32); 3] {
        match self {
            Stock::Hardwood => [(0.003, 0.005), (0.009, 0.011), (0.019, 0.021)],
            Stock::Softwood => [(0.004, 0.006), (0.011, 0.013), (0.021, 0.023)],
            Stock::Mdf => [(0.004, 0.007), (0.013, 0.016), (0.025, 0.027)],
            Stock::Plywood => [(0.003, 0.006), (0.010, 0.012), (0.020, 0.022)],
            Stock::Acrylic => [(0.003, 0.005), (0.006, 0.008), (0.010, 0.012)],
            Stock::Hdpe => [(0.003, 0.006), (0.007, 0.009), (0.012, 0.016)],
            Stock::Aluminium => [(0.002, 0.003), (0.003, 0.004), (0.008, 0.010)],
        }
    }

    /// The published chip-load range (mm per tooth) for a cutting diameter, interpolated
    /// between the charted sizes (smaller tools scale down, bigger ones grow slowly).
    pub fn published_range(self, d: f32) -> (f32, f32) {
        let t = self.published_inches();
        let xs = [3.175f32, 6.35, 12.7];
        let at = |i: usize| (t[i].0 * 25.4, t[i].1 * 25.4);
        if d <= xs[0] {
            let (a, b) = at(0);
            let k = d / xs[0];
            (a * k, b * k)
        } else if d >= xs[2] {
            let (a, b) = at(2);
            let k = (d / xs[2]).sqrt();
            (a * k, b * k)
        } else {
            let i = if d <= xs[1] { 0 } else { 1 };
            let f = (d - xs[i]) / (xs[i + 1] - xs[i]);
            let (a0, b0) = at(i);
            let (a1, b1) = at(i + 1);
            (a0 + (a1 - a0) * f, b0 + (b1 - b0) * f)
        }
    }
    /// Cutting surface speed (m/min) the material likes.
    fn surface_speed(self) -> f32 {
        match self {
            Stock::Softwood | Stock::Mdf => 600.0,
            Stock::Hardwood | Stock::Plywood => 500.0,
            Stock::Hdpe => 400.0,
            Stock::Acrylic => 300.0,
            Stock::Aluminium => 200.0,
        }
    }
    /// Pass depth as a fraction of the tool diameter.
    fn depth_factor(self) -> f32 {
        match self {
            Stock::Softwood | Stock::Mdf | Stock::Hdpe => 1.0,
            Stock::Plywood => 0.75,
            Stock::Hardwood | Stock::Acrylic => 0.5,
            Stock::Aluminium => 0.2,
        }
    }
}

/// How stiff the machine is: scales chip load and pass depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Rigidity {
    Hobby,
    #[default]
    Standard,
    Industrial,
}

impl Rigidity {
    pub const ALL: [Rigidity; 3] = [Rigidity::Hobby, Rigidity::Standard, Rigidity::Industrial];
    pub fn label(self) -> &'static str {
        match self {
            Rigidity::Hobby => "Hobby / light frame",
            Rigidity::Standard => "Standard",
            Rigidity::Industrial => "Industrial / heavy frame",
        }
    }
    fn factor(self) -> f32 {
        match self {
            Rigidity::Hobby => 0.6,
            Rigidity::Standard => 1.0,
            Rigidity::Industrial => 1.25,
        }
    }
    /// Where in the published chip-load range to aim: a light frame stays below it (to
    /// avoid chatter), a standard machine near its low end, a heavy one mid-range.
    fn chip_position(self, (lo, hi): (f32, f32)) -> f32 {
        match self {
            Rigidity::Hobby => lo * 0.6,
            Rigidity::Standard => lo * 0.9,
            Rigidity::Industrial => (lo + hi) * 0.5,
        }
    }
}

/// The machine's limits, saved with the tool library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Machine {
    pub min_rpm: f32,
    pub max_rpm: f32,
    pub max_feed: f32,
    pub rigidity: Rigidity,
}

impl Default for Machine {
    fn default() -> Self {
        Machine { min_rpm: 8000.0, max_rpm: 24000.0, max_feed: 5000.0, rigidity: Rigidity::Standard }
    }
}

/// What the calculator recommends for a tool.
#[derive(Clone, Debug, PartialEq)]
pub struct Advice {
    pub rpm: f32,
    pub feed: f32,
    pub plunge: f32,
    pub pass_depth: f32,
    pub stepover: f32,
    pub chip_load: f32,
    /// Published chip-load range for this tool size and material (mm per tooth).
    pub published: (f32, f32),
    pub notes: Vec<String>,
}

/// Recommended settings for `tool` cutting `stock` on `machine`.
pub fn advise(tool: &Tool, stock: Stock, machine: &Machine) -> Advice {
    let mut notes = Vec::new();
    // A V-bit mostly cuts with the narrow part near its tip.
    let cutting_d = match tool.kind {
        ToolKind::VBit => (tool.diameter * 0.5).max(0.5),
        _ => tool.diameter.max(0.1),
    };
    let flutes = tool.flutes.max(1) as f32;
    let rig = machine.rigidity.factor();
    let published = stock.published_range(cutting_d);
    let chip = machine.rigidity.chip_position(published);
    let ideal_rpm = stock.surface_speed() * 1000.0 / (std::f32::consts::PI * cutting_d);
    let rpm = ideal_rpm.clamp(machine.min_rpm, machine.max_rpm).round();
    if ideal_rpm > machine.max_rpm * 1.05 {
        notes.push(format!("This size would like about {:.0} r.p.m; capped at your spindle's {:.0}.", ideal_rpm, machine.max_rpm));
    }
    let mut feed = rpm * flutes * chip;
    if feed > machine.max_feed {
        notes.push(format!("Feed capped at your machine's {:.0} mm/min (chip load falls to {:.3} mm).", machine.max_feed, machine.max_feed / (rpm * flutes)));
        feed = machine.max_feed;
    }
    let real_chip = feed / (rpm * flutes);
    if real_chip < 0.01 {
        notes.push("Very small chip load: the tool may rub and burn. Use fewer flutes, a lower spindle speed or a faster feed.".into());
    }
    let plunge = feed * if stock == Stock::Aluminium { 0.2 } else { 0.3 };
    let (pass_depth, stepover) = match tool.kind {
        ToolKind::EndMill => (tool.diameter * stock.depth_factor() * rig, tool.diameter * 0.4),
        ToolKind::BallNose => (tool.diameter * stock.depth_factor() * rig * 0.5, tool.diameter * 0.1),
        ToolKind::VBit => (tool.diameter * 0.25 * stock.depth_factor().min(1.0), (tool.diameter * 0.05).max(0.05)),
    };
    Advice {
        rpm,
        feed: feed.round(),
        plunge: plunge.round(),
        pass_depth: (pass_depth * 100.0).round() / 100.0,
        stepover: (stepover * 1000.0).round() / 1000.0,
        chip_load: real_chip,
        published,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end_mill(d: f32) -> Tool {
        Tool::new(1, "EM", ToolKind::EndMill, d, 0.0, 1.0, 1.0, 1000.0)
    }

    #[test]
    fn quarter_inch_end_mill_in_hardwood_is_sensible() {
        let a = advise(&end_mill(6.35), Stock::Hardwood, &Machine::default());
        // 2 flutes at about 0.15 mm per tooth: 6000+ mm/min wanted, so capped at 5000
        assert_eq!(a.rpm, 24000.0);
        assert_eq!(a.feed, 5000.0);
        assert!(a.notes.iter().any(|n| n.contains("capped")));
        assert!((a.pass_depth - 3.18).abs() < 0.02 && (a.stepover - 2.54).abs() < 0.01);
    }

    #[test]
    fn harder_materials_and_lighter_machines_go_slower() {
        let m = Machine { max_feed: 20000.0, ..Machine::default() };
        let soft = advise(&end_mill(3.0), Stock::Softwood, &m);
        let alu = advise(&end_mill(3.0), Stock::Aluminium, &m);
        assert!(alu.feed < soft.feed / 2.0 && alu.rpm < soft.rpm);
        let hobby = advise(&end_mill(3.0), Stock::Softwood, &Machine { rigidity: Rigidity::Hobby, ..m.clone() });
        assert!(hobby.feed < soft.feed && hobby.pass_depth < soft.pass_depth);
    }

    /// Every material and size lands where the published charts say, for each machine type.
    #[test]
    fn chip_loads_match_published_ranges() {
        for stock in Stock::ALL {
            for d in [3.175f32, 4.0, 6.35, 9.525, 12.7] {
                let (lo, hi) = stock.published_range(d);
                let unlimited = |r| Machine { min_rpm: 1000.0, max_rpm: 18000.0, max_feed: 100000.0, rigidity: r };
                let chip = |r| advise(&end_mill(d), stock, &unlimited(r)).chip_load;
                let (ind, std, hob) = (chip(Rigidity::Industrial), chip(Rigidity::Standard), chip(Rigidity::Hobby));
                assert!(ind >= lo - 1e-4 && ind <= hi + 1e-4, "{stock:?} {d} mm industrial {ind} not in {lo}-{hi}");
                assert!(std >= lo * 0.85 && std <= hi, "{stock:?} {d} mm standard {std} vs {lo}-{hi}");
                assert!(hob < std && hob >= lo * 0.5, "{stock:?} {d} mm hobby {hob}");
            }
        }
        // spot check against the charts: 1/4" in hardwood is 0.009-0.011" per tooth
        let (lo, hi) = Stock::Hardwood.published_range(6.35);
        assert!((lo / 25.4 - 0.009).abs() < 1e-5 && (hi / 25.4 - 0.011).abs() < 1e-5);
    }

    #[test]
    fn material_names_map_to_stock() {
        assert_eq!(Stock::from_name("Hardwood"), Stock::Hardwood);
        assert_eq!(Stock::from_name("MDF"), Stock::Mdf);
        assert_eq!(Stock::from_name("Aluminium"), Stock::Aluminium);
        assert_eq!(Stock::from_name("Default"), Stock::Softwood);
    }
}
