extern crate failure;
#[macro_use]
extern crate log;
extern crate osmpbfreader;
#[macro_use]
extern crate serde_derive;

mod zone;
mod admin_type;
pub mod cosmogony;

use std::fs::File;
use std::path::Path;
use cosmogony::{Cosmogony, CosmogonyMetadata, CosmogonyStats};
use osmpbfreader::{OsmObj, OsmPbfReader};

use failure::Error;
use failure::ResultExt;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub fn is_admin(obj: &OsmObj) -> bool {
    match *obj {
        OsmObj::Relation(ref rel) => {
            rel.tags
                .get("boundary")
                .map_or(false, |v| v == "administrative")
            &&
            rel.tags.get("admin_level").is_some()
        }
        _ => false,
    }
}

pub fn get_zones_and_stats(
    pbf: &mut OsmPbfReader<File>,
) -> Result<(Vec<zone::Zone>, CosmogonyStats), Error> {
    info!("Reading pbf with geometries...");
    let objects = pbf.get_objs_and_deps(|o| is_admin(o))
        .context("invalid osm file")?;
    info!("reading pbf done.");

    let mut zones = vec![];
    let mut stats = CosmogonyStats::default();

    for obj in objects.values() {
        if !is_admin(obj) {
            continue;
        }
        if let OsmObj::Relation(ref relation) = *obj {
            if let Some(zone) = zone::Zone::from_osm_with_geom(relation, &objects) {
                // Ignore zone without boundary polygon
                if zone.boundary.is_some() {
                    stats.process(&zone);
                    zones.push(zone);
                }
            }
        }
    }

    return Ok((zones, stats));
}

pub fn get_zones_and_stats_without_geom(
    pbf: &mut OsmPbfReader<File>,
) -> Result<(Vec<zone::Zone>, CosmogonyStats), Error> {
    info!("Reading pbf without geometries...");

    let mut zones = vec![];
    let mut stats = CosmogonyStats::default();

    for obj in pbf.par_iter().map(Result::unwrap) {
        if !is_admin(&obj) {
            continue;
        }
        if let OsmObj::Relation(ref relation) = obj {
            if let Some(zone) = zone::Zone::from_osm(relation) {
                stats.process(&zone);
                zones.push(zone);
            }
        }
    }

    Ok((zones, stats))
}

pub fn build_cosmogony(pbf_path: String, with_geom: bool) -> Result<Cosmogony, Error> {
    let path = Path::new(&pbf_path);
    let file = File::open(&path).context("no pbf file")?;

    let mut parsed_pbf = OsmPbfReader::new(file);

    let (zones, stats) = if with_geom {
        get_zones_and_stats(&mut parsed_pbf)?
    } else {
        get_zones_and_stats_without_geom(&mut parsed_pbf)?
    };
    let cosmogony = Cosmogony {
        zones: zones,
        meta: CosmogonyMetadata {
            osm_filename: path.file_name()
                .and_then(|f| f.to_str())
                .map(|f| f.to_string())
                .unwrap_or("invalid file name".into()),
            stats: stats,
        },
    };
    Ok(cosmogony)
}
