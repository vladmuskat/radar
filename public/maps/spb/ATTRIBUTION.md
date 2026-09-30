# Санкт-Петербург: офлайн-карта

© OpenStreetMap contributors.

This derived map database is made available under the Open Database License (ODbL) 1.0:
https://opendatacommons.org/licenses/odbl/1-0/

Source and attribution: https://www.openstreetmap.org/copyright

Источник: предоставленный пользователем `northwestern-fed-district-260928.osm.pbf`,
из которого Osmium вырезал `spb-industrial.osm.pbf`. Дата 2026-09-28 взята из имени
выгрузки, не является гарантией актуальности каждого объекта OSM. Региональные
выгрузки: https://download.geofabrik.de/russia/northwestern-fed-district.html

Изменения: отбор картографических тегов, сборка мультиполигонов, обрезка до
30.08,59.79,30.44,59.97, удаление лишних атрибутов, упрощение геометрии,
восстановление морской заливки по направленной береговой линии, преобразование
в локальные векторные тайлы MVT. Координаты и хеш исходного файла — в manifest.json.

Производная база распространяется в машиночитаемом виде в соседнем каталоге
`tiles/`, также под ODbL 1.0. Сохраняйте этот файл вместе с тайлами при передаче.
Метод воспроизведения: `scripts/prepare-map.ps1`, `scripts/prepare_map.py`,
`scripts/build-map-tiles.mjs` и `docs/MAP_DATA.md` в исходниках приложения.

Положение РЛС, зоны и цели — учебные, не сведения о реальной системе охраны.
Карта не предназначена для навигации или принятия эксплуатационных решений.
