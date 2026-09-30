"""Build redistributable offline layers from an Osmium GeoJSON export.

Run via uv with Shapely 2.1.2. Never modifies the source PBF/export.
All files written here are generated data, not application source code.
"""
import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path

from shapely import make_valid, normalize, set_precision
from shapely.geometry import LineString, Point, box, mapping, shape
from shapely.ops import polygonize, unary_union
from shapely.strtree import STRtree

ROOT = Path(__file__).resolve().parent.parent.parent
REGION = json.loads((ROOT / "map-region.json").read_text(encoding="utf-8"))
ROAD_TYPES = {
    "motorway", "motorway_link", "trunk", "trunk_link", "primary", "primary_link",
    "secondary", "secondary_link", "tertiary", "tertiary_link", "residential",
    "unclassified", "service", "living_street", "pedestrian",
}


def parts(geometry, dimension):
    """Flatten collections after clipping without losing polygon interior rings."""
    if geometry.is_empty:
        return
    if dimension == 2 and geometry.geom_type == "Polygon":
        yield geometry
    elif dimension == 1 and geometry.geom_type == "LineString":
        yield geometry
    elif hasattr(geometry, "geoms"):
        for child in geometry.geoms:
            yield from parts(child, dimension)


def ocean_polygons(coastlines, bounds):
    """Polygonize the coastline with the viewport; OSM land is on the left.

    Assign the adjacent cells using the direction of each original coastline
    segment. Fail on contradictory/unclosed coastline rather than invent a coast.
    """
    if not coastlines:
        raise ValueError("No coastline found: coastal basemap requires explicit sea geometry")
    cells = list(polygonize(unary_union([bounds.boundary, *coastlines])))
    tree = STRtree(cells)
    labels = [set() for _ in cells]
    for coast in coastlines:
        for a, b in zip(coast.coords, list(coast.coords)[1:]):
            dx, dy = b[0] - a[0], b[1] - a[1]
            length = math.hypot(dx, dy)
            if length < 1e-9:
                continue
            epsilon = min(length / 10, 1e-7)
            mid = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
            for side, label in [(1, "land"), (-1, "water")]:
                probe = Point(mid[0] - side * dy / length * epsilon,
                              mid[1] + side * dx / length * epsilon)
                for index in tree.query(probe, predicate="within"):
                    labels[index].add(label)
    if any(len(label) != 1 for label in labels):
        raise ValueError(f"Incomplete/ambiguous coastline cells: {[i for i, x in enumerate(labels) if len(x) != 1]}")
    return [cell for cell, label in zip(cells, labels) if label == {"water"}]


def classify(tags, geometry):
    """Return the output layer, visual kind and geometry dimension for an OSM feature."""
    area = geometry.geom_type in ("Polygon", "MultiPolygon")
    if tags.get("natural") == "coastline":
        return ("coastline", "coastline", 1) if not area else None
    if area:
        if tags.get("building") not in (None, "no"):
            return "buildings", "building", 2
        if tags.get("natural") == "water" or "water" in tags or tags.get("waterway") == "riverbank":
            return "water", "water", 2
        landuse = tags.get("landuse")
        if landuse in ("industrial", "commercial", "retail", "railway", "construction", "residential"):
            return "landuse", landuse, 2
        if landuse in ("forest", "grass", "meadow", "recreation_ground", "cemetery", "allotments") or tags.get("natural") in ("wood", "scrub", "wetland") or tags.get("leisure") in ("park", "garden"):
            return "landuse", "green", 2
    else:
        if tags.get("highway") in ROAD_TYPES:
            return "roads", tags["highway"], 1
        if tags.get("railway") in ("rail", "tram", "light_rail", "narrow_gauge") and tags.get("tunnel") not in ("yes", "building_passage"):
            return "railways", tags["railway"], 1
        if tags.get("waterway") in ("river", "canal", "stream", "ditch"):
            return "water", "waterway", 1
    return None


def generate(export_path, source_path, output):
    """Validate, clip and write deterministic GeoJSON layers plus their manifest."""
    bounds = box(*REGION["bbox"])
    layers = {name: [] for name in ["buildings", "roads", "railways", "water", "landuse"]}
    coastlines, counters = [], Counter()
    data = json.loads(export_path.read_text(encoding="utf-8"))
    for feature in data["features"]:
        geometry = shape(feature["geometry"])
        category = classify(feature["properties"], geometry)
        if not category or not geometry.intersects(bounds):
            continue
        if not geometry.is_valid:
            geometry = make_valid(geometry)
            counters["repaired"] += 1
        name, kind, dimension = category
        clipped = geometry.intersection(bounds)
        if name == "coastline":
            coastlines.extend(parts(clipped, 1))
            continue
        for index, part in enumerate(parts(clipped, dimension)):
            # Sub-metre simplification; snapping ensures bounded, stable file sizes.
            part = set_precision(part.simplify(0.000005, preserve_topology=True), 0.000001)
            for subindex, simple in enumerate(parts(part, dimension)):
                if simple.is_empty:
                    continue
                layers[name].append({"type": "Feature", "id": f"{feature['id']}-{index}-{subindex}",
                                     "properties": {"kind": kind}, "geometry": mapping(simple)})
    ocean = sorted((normalize(g) for g in ocean_polygons(coastlines, bounds)), key=lambda g: (g.bounds, g.wkb_hex))
    for index, polygon in enumerate(ocean):
        polygon = normalize(set_precision(polygon.simplify(0.000005, preserve_topology=True), 0.000001))
        layers["water"].append({"type": "Feature", "id": f"sea-{index}",
                                "properties": {"kind": "sea"}, "geometry": mapping(polygon)})
    output.mkdir(parents=True, exist_ok=True)
    summary = {}
    for name, features in layers.items():
        if not features:
            raise ValueError(f"Empty layer: {name}")
        payload = json.dumps({"type": "FeatureCollection", "features": features}, ensure_ascii=False, separators=(",", ":"))
        destination = output / f"{name}.geojson"
        destination.write_text(payload, encoding="utf-8")
        summary[name] = {"features": len(features), "bytes": destination.stat().st_size,
                         "sha256": hashlib.sha256(destination.read_bytes()).hexdigest()}
    metadata = {"region": REGION, "source": source_path.name,
                "sourceSha256": hashlib.sha256(source_path.read_bytes()).hexdigest(),
                "license": "ODbL-1.0", "attribution": "© OpenStreetMap contributors",
                "generator": "osmium export + scripts/map/prepare_map.py (Shapely 2.1.2)",
                "geometryRepairs": counters["repaired"], "layers": summary}
    (output / "manifest.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(metadata, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    generate(args.input, args.source, args.output)
