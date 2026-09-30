import unittest
from shapely.geometry import LineString, Polygon, Point, box
from prepare_map import ocean_polygons, parts


class GeometryTests(unittest.TestCase):
    def test_directed_shoreline_keeps_water_on_right(self):
        # Eastward coast: north is land, south is sea.
        water = ocean_polygons([LineString([(0, 5), (10, 5)])], box(0, 0, 10, 10))
        self.assertEqual(len(water), 1)
        self.assertTrue(water[0].contains(Point(5, 2)))
        self.assertFalse(water[0].contains(Point(5, 8)))

    def test_island_is_a_hole_in_sea(self):
        # Counter-clockwise coast surrounds the land with sea to its right.
        island = LineString([(3, 3), (7, 3), (7, 7), (3, 7), (3, 3)])
        water = ocean_polygons([island], box(0, 0, 10, 10))
        self.assertEqual(len(water), 1)
        self.assertEqual(len(water[0].interiors), 1)
        self.assertFalse(water[0].contains(Point(5, 5)))

    def test_unclosed_shoreline_is_rejected(self):
        with self.assertRaises(ValueError):
            ocean_polygons([LineString([(2, 5), (8, 5)])], box(0, 0, 10, 10))

    def test_clipping_preserves_polygon_holes(self):
        building = Polygon([(0, 0), (10, 0), (10, 10), (0, 10)],
                           holes=[[(3, 3), (3, 7), (7, 7), (7, 3)]])
        clipped = list(parts(building.intersection(box(1, 1, 9, 9)), 2))
        self.assertEqual(len(clipped[0].interiors), 1)
        self.assertAlmostEqual(clipped[0].area, 48)


if __name__ == '__main__':
    unittest.main()
