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
    /// Chip load per tooth (mm) at 1.5, 3, 6 and 12 mm cutting diameter.
    fn chip_table(self) -> [f32; 4] {
        match self {
            Stock::Softwood => [0.025, 0.06, 0.17, 0.30],
            Stock::Hardwood => [0.02, 0.05, 0.14, 0.25],
            Stock::Mdf => [0.03, 0.08, 0.20, 0.33],
            Stock::Plywood => [0.025, 0.065, 0.17, 0.30],
            Stock::Acrylic => [0.025, 0.06, 0.15, 0.25],
            Stock::Hdpe => [0.03, 0.08, 0.20, 0.35],
            Stock::Aluminium => [0.008, 0.02, 0.05, 0.10],
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
    pub notes: Vec<String>,
}

fn lerp_table(t: [f32; 4], d: f32) -> f32 {
    let xs = [1.5f32, 3.0, 6.0, 12.0];
    if d <= xs[0] {
        return t[0] * d / xs[0];
    }
    for i in 0..3 {
        if d <= xs[i + 1] {
            let f = (d - xs[i]) / (xs[i + 1] - xs[i]);
            return t[i] + (t[i + 1] - t[i]) * f;
        }
    }
    t[3] * (d / 12.0).sqrt()
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
    let chip = lerp_table(stock.chip_table(), cutting_d) * rig;
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

    #[test]
    fn material_names_map_to_stock() {
        assert_eq!(Stock::from_name("Hardwood"), Stock::Hardwood);
        assert_eq!(Stock::from_name("MDF"), Stock::Mdf);
        assert_eq!(Stock::from_name("Aluminium"), Stock::Aluminium);
        assert_eq!(Stock::from_name("Default"), Stock::Softwood);
    }
}
