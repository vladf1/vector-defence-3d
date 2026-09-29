import path from "node:path";
import { repoRoot, runBrowserPage, waitForPageResult, writeDataUrlPngMap } from "./benchmark-browser-harness.mjs";

const outputDir = path.resolve(repoRoot, "artifacts/polygon-shards");

const html = String.raw`
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>Polygon Shard Splitter</title>
    <style>
      body {
        margin: 0;
        background: #050807;
      }
      canvas {
        display: block;
      }
    </style>
  </head>
  <body>
    <canvas id="sheet"></canvas>
    <script type="module">
      const { PackManMonster } = await import("/src/entities/monsters/packman-monster.ts");
      const { PolygonShardSplitter, createPolygonShardSplitterConfig } = await import("/src/entities/monsters/polygon-shard-splitter.ts");

      const CELL_WIDTH = 390;
      const CELL_HEIGHT = 250;
      const COLUMN_COUNT = 3;
      const VARIANT_COUNT = 3;
      const SHEET_WIDTH = CELL_WIDTH * COLUMN_COUNT;
      const SHEET_HEIGHT = CELL_HEIGHT * VARIANT_COUNT;
      const EDGE_EPSILON = 0.025;

      const specimens = [
        {
          slug: "pacman",
          label: "Pac-Man",
          color: "#5df2ef",
          seeds: [3001, 3002, 3003],
          outline: createPacManOutline(18),
          configOverrides: {
            maxShardVertices: 26,
          },
        },
        {
          slug: "square",
          label: "Square",
          color: "#ff6f62",
          seeds: [3101, 3102, 3103],
          outline: squareOutline(6.5),
        },
        {
          slug: "triangle",
          label: "Triangle",
          color: "#ffba4f",
          seeds: [3201, 3202, 3203],
          outline: [
            point(6, 0),
            point(-6, -6),
            point(-6, 6),
          ],
        },
        {
          slug: "runner",
          label: "Runner",
          color: "#91ff63",
          seeds: [3301, 3302, 3303],
          outline: [
            point(5.5 * 1.7, 0),
            point(-5.5 * 0.1, -5.5 * 0.82),
            point(-5.5 * 1.35, 0),
            point(-5.5 * 0.1, 5.5 * 0.82),
          ],
        },
        {
          slug: "splitter",
          label: "Splitter",
          color: "#ff8bd5",
          seeds: [3401, 3402, 3403],
          outline: splitterOutline(8.5),
        },
        {
          slug: "tank-hull",
          label: "Tank Hull",
          color: "#9fb6ff",
          seeds: [3501, 3502, 3503],
          outline: tankHullOutline(10.5),
        },
        {
          slug: "bulwark-shell",
          label: "Bulwark Shell",
          color: "#dff7ff",
          seeds: [3601, 3602, 3603],
          outline: bulwarkShellOutline(9.5),
        },
        {
          slug: "bulwark-core",
          label: "Bulwark Core",
          color: "#78d7ff",
          seeds: [3701, 3702, 3703],
          outline: bulwarkCoreOutline(9.5),
        },
        {
          slug: "berserker",
          label: "Berserker",
          color: "#ff3158",
          seeds: [3801, 3802, 3803],
          outline: berserkerOutline(8),
        },
      ];

      const sheetCanvas = document.getElementById("sheet");
      const sheetContext = sheetCanvas.getContext("2d");
      sheetCanvas.width = SHEET_WIDTH;
      sheetCanvas.height = SHEET_HEIGHT;

      const outputs = {};

      for (const specimen of specimens) {
        const renderedVariants = [];
        for (let variantIndex = 0; variantIndex < VARIANT_COUNT; variantIndex += 1) {
          const seed = specimen.seeds[variantIndex];
          const config = createSplitterConfig(
            createSeededRandom(seed),
            specimen.configOverrides ?? {},
          );
          const splitter = new PolygonShardSplitter(config);
          const shards = splitter.splitIntoShards(specimen.outline);
          assertShardSet(specimen, shards, config);
          renderedVariants.push({ seed, shards });
        }

        sheetContext.fillStyle = "#050807";
        sheetContext.fillRect(0, 0, SHEET_WIDTH, SHEET_HEIGHT);

        for (let variantIndex = 0; variantIndex < renderedVariants.length; variantIndex += 1) {
          const variant = renderedVariants[variantIndex];
          drawOriginalCell(sheetContext, specimen, variantIndex, variant.seed);
          drawCrackCell(sheetContext, specimen, variant.shards, variantIndex, variant.seed);
          drawShardCell(sheetContext, specimen, variant.shards, variantIndex, variant.seed);
        }

        outputs[specimen.slug + "-polygon-shards"] = sheetCanvas.toDataURL("image/png");
      }

      window.__polygonShardRender = outputs;

      function createSplitterConfig(random, overrides) {
        return createPolygonShardSplitterConfig({
          minShardCount: 5,
          maxShardCount: 11,
          random,
          ...overrides,
        });
      }

      function drawOriginalCell(context, specimen, rowIndex, seed) {
        drawCellBase(context, specimen, 0, rowIndex, seed, "original");
        drawPolygon(context, specimen.outline, {
          fill: "rgba(255, 255, 255, 0.035)",
          stroke: specimen.color,
          lineWidth: 0.12,
        });
        context.restore();
      }

      function drawCrackCell(context, specimen, shards, rowIndex, seed) {
        drawCellBase(context, specimen, 1, rowIndex, seed, "crack graph");
        drawPolygon(context, specimen.outline, {
          fill: "rgba(255, 255, 255, 0.025)",
          stroke: "rgba(235, 255, 247, 0.34)",
          lineWidth: 0.08,
        });

        const crackSegments = collectInternalSegments(shards);
        context.save();
        context.strokeStyle = "rgba(255, 255, 255, 0.9)";
        context.lineWidth = 0.075;
        context.lineCap = "round";
        context.lineJoin = "round";
        for (const segment of crackSegments) {
          context.beginPath();
          context.moveTo(segment.start.x, segment.start.y);
          context.lineTo(segment.end.x, segment.end.y);
          context.stroke();
        }
        context.restore();

        const dots = collectInteriorDots(crackSegments, specimen.outline);
        context.save();
        context.fillStyle = "#050807";
        context.strokeStyle = "#ffffff";
        context.lineWidth = 0.06;
        for (const dot of dots) {
          context.beginPath();
          context.arc(dot.x, dot.y, 0.19, 0, Math.PI * 2);
          context.fill();
          context.stroke();
        }
        context.restore();
        context.restore();
      }

      function drawShardCell(context, specimen, shards, rowIndex, seed) {
        drawCellBase(context, specimen, 2, rowIndex, seed, shards.length + " shards");
        for (let index = 0; index < shards.length; index += 1) {
          drawPolygon(context, shards[index].vertices, {
            fill: shardFill(specimen.color, index),
            stroke: index % 2 === 0 ? "rgba(255, 255, 255, 0.86)" : specimen.color,
            lineWidth: 0.09,
          });
        }
        context.restore();
      }

      function drawCellBase(context, specimen, columnIndex, rowIndex, seed, caption) {
        const x = columnIndex * CELL_WIDTH;
        const y = rowIndex * CELL_HEIGHT;
        context.save();
        context.fillStyle = "#050807";
        context.fillRect(x, y, CELL_WIDTH, CELL_HEIGHT);
        context.strokeStyle = "rgba(255, 255, 255, 0.07)";
        context.lineWidth = 1;
        context.strokeRect(x + 0.5, y + 0.5, CELL_WIDTH - 1, CELL_HEIGHT - 1);
        context.fillStyle = "rgba(238, 255, 247, 0.94)";
        context.font = "900 17px Avenir Next, Arial Black, Arial, sans-serif";
        context.fillText(specimen.label, x + 18, y + 27);
        context.font = "700 13px Avenir Next, Arial, sans-serif";
        context.fillStyle = "rgba(238, 255, 247, 0.72)";
        context.fillText(caption + " / seed " + seed, x + 18, y + 47);

        const bounds = polygonBounds(specimen.outline);
        const scale = Math.min(
          (CELL_WIDTH - 96) / (bounds.maxX - bounds.minX),
          (CELL_HEIGHT - 82) / (bounds.maxY - bounds.minY),
        );
        context.translate(x + CELL_WIDTH / 2, y + CELL_HEIGHT / 2 + 18);
        context.scale(scale, scale);
        context.translate(
          -(bounds.minX + bounds.maxX) / 2,
          -(bounds.minY + bounds.maxY) / 2,
        );
      }

      function drawPolygon(context, polygon, style) {
        context.save();
        context.beginPath();
        context.moveTo(polygon[0].x, polygon[0].y);
        for (let index = 1; index < polygon.length; index += 1) {
          context.lineTo(polygon[index].x, polygon[index].y);
        }
        context.closePath();
        context.fillStyle = style.fill;
        context.strokeStyle = style.stroke;
        context.lineWidth = style.lineWidth;
        context.fill();
        context.stroke();
        context.restore();
      }

      function assertShardSet(specimen, shards, config) {
        if (shards.length < config.minShardCount || shards.length > config.maxShardCount) {
          throw new Error(specimen.label + " produced " + shards.length + " shards");
        }

        for (const shard of shards) {
          if (shard.vertices.length < 3) {
            throw new Error(specimen.label + " produced a shard with fewer than 3 vertices");
          }
        }

        const sourceArea = polygonArea(specimen.outline);
        const shardArea = shards.reduce((sum, shard) => sum + polygonArea(shard.vertices), 0);
        const areaDeltaRatio = Math.abs(shardArea - sourceArea) / sourceArea;
        if (areaDeltaRatio > config.areaToleranceRatio) {
          throw new Error(
            specimen.label + " shard area drifted by " + areaDeltaRatio.toFixed(4),
          );
        }

        const largestShardAreaRatio = Math.max(
          ...shards.map((shard) => polygonArea(shard.vertices) / sourceArea),
        );
        if (largestShardAreaRatio > config.maxShardAreaRatio) {
          throw new Error(
            specimen.label + " largest shard is " + largestShardAreaRatio.toFixed(4) + " of the source",
          );
        }
      }

      function collectInternalSegments(shards) {
        const edges = new Map();
        for (const shard of shards) {
          for (let index = 0; index < shard.vertices.length; index += 1) {
            const start = shard.vertices[index];
            const end = shard.vertices[(index + 1) % shard.vertices.length];
            const key = segmentKey(start, end);
            const existing = edges.get(key);
            if (existing === undefined) {
              edges.set(key, { start, end, count: 1 });
            } else {
              existing.count += 1;
            }
          }
        }

        return Array.from(edges.values()).filter((edge) => edge.count > 1);
      }

      function collectInteriorDots(segments, outline) {
        const points = new Map();
        for (const segment of segments) {
          points.set(pointKey(segment.start), segment.start);
          points.set(pointKey(segment.end), segment.end);
        }

        return Array.from(points.values()).filter(
          (point) => distanceToPolygonBoundary(point, outline) > EDGE_EPSILON * 3,
        );
      }

      function segmentKey(start, end) {
        const startKey = pointKey(start);
        const endKey = pointKey(end);
        return startKey < endKey ? startKey + "|" + endKey : endKey + "|" + startKey;
      }

      function pointKey(point) {
        return Math.round(point.x * 10000) + "," + Math.round(point.y * 10000);
      }

      function createPacManOutline(arcVertexCount) {
        const path = [
          { x: 0, y: 0, totalDistance: 0 },
          { x: 100, y: 0, totalDistance: 100 },
        ];
        const monster = new PackManMonster(path, 1);
        return monster.createOutline(arcVertexCount);
      }

      function squareOutline(radius) {
        return [
          point(-radius, -radius),
          point(radius, -radius),
          point(radius, radius),
          point(-radius, radius),
        ];
      }

      function splitterOutline(radius) {
        return Array.from({ length: 6 }, (_, index) => {
          const angle = (Math.PI / 3) * index;
          const pointRadius = index % 2 === 0 ? radius * 1.15 : radius * 0.72;
          return point(Math.cos(angle) * pointRadius, Math.sin(angle) * pointRadius);
        });
      }

      function tankHullOutline(radius) {
        const x = -radius;
        const y = -radius * 0.72;
        const width = radius * 2.1;
        const height = radius * 1.44;
        return [
          point(x, y),
          point(x + width, y),
          point(x + width, y + height),
          point(x, y + height),
        ];
      }

      function bulwarkShellOutline(radius) {
        const halfHeight = radius * 0.8;
        return [
          point(radius * 1.35, 0),
          point(radius * 0.82, -halfHeight),
          point(-radius * 0.2, -radius * 0.98),
          point(-radius * 1.08, -halfHeight),
          point(-radius * 1.32, 0),
          point(-radius * 1.08, halfHeight),
          point(-radius * 0.2, radius * 0.98),
          point(radius * 0.82, halfHeight),
        ];
      }

      function bulwarkCoreOutline(radius) {
        return [
          point(radius * 0.98, 0),
          point(radius * 0.42, -radius * 0.46),
          point(-radius * 0.3, -radius * 0.46),
          point(-radius * 0.72, 0),
          point(-radius * 0.3, radius * 0.46),
          point(radius * 0.42, radius * 0.46),
        ];
      }

      function berserkerOutline(radius) {
        return [
          point(radius * 1.55, 0),
          point(radius * 0.4, -radius * 0.8),
          point(-radius * 0.1, -radius * 1.08),
          point(-radius * 1.28, -radius * 0.44),
          point(-radius * 0.72, 0),
          point(-radius * 1.28, radius * 0.44),
          point(-radius * 0.1, radius * 1.08),
          point(radius * 0.4, radius * 0.8),
        ];
      }

      function polygonArea(polygon) {
        let total = 0;
        for (let index = 0; index < polygon.length; index += 1) {
          const current = polygon[index];
          const next = polygon[(index + 1) % polygon.length];
          total += (current.x * next.y) - (next.x * current.y);
        }
        return Math.abs(total) / 2;
      }

      function polygonBounds(polygon) {
        let minX = polygon[0].x;
        let maxX = polygon[0].x;
        let minY = polygon[0].y;
        let maxY = polygon[0].y;
        for (const point of polygon) {
          minX = Math.min(minX, point.x);
          maxX = Math.max(maxX, point.x);
          minY = Math.min(minY, point.y);
          maxY = Math.max(maxY, point.y);
        }
        return { minX, maxX, minY, maxY };
      }

      function distanceToPolygonBoundary(point, polygon) {
        let minDistance = Number.POSITIVE_INFINITY;
        for (let index = 0; index < polygon.length; index += 1) {
          minDistance = Math.min(
            minDistance,
            distanceToSegment(point, polygon[index], polygon[(index + 1) % polygon.length]),
          );
        }
        return minDistance;
      }

      function distanceToSegment(point, start, end) {
        const dx = end.x - start.x;
        const dy = end.y - start.y;
        const lengthSquared = (dx * dx) + (dy * dy);
        if (lengthSquared === 0) {
          return Math.hypot(point.x - start.x, point.y - start.y);
        }

        const ratio = Math.min(
          1,
          Math.max(0, (((point.x - start.x) * dx) + ((point.y - start.y) * dy)) / lengthSquared),
        );
        return Math.hypot(
          point.x - (start.x + (dx * ratio)),
          point.y - (start.y + (dy * ratio)),
        );
      }

      function shardFill(hex, index) {
        const rgb = hexToRgb(hex);
        const tint = 0.55 + ((index % 5) * 0.1);
        return "rgba(" +
          Math.round(rgb.r * tint + 255 * (1 - tint)) + ", " +
          Math.round(rgb.g * tint + 255 * (1 - tint)) + ", " +
          Math.round(rgb.b * tint + 255 * (1 - tint)) + ", 0.72)";
      }

      function hexToRgb(hex) {
        const clean = hex.replace("#", "");
        return {
          r: parseInt(clean.slice(0, 2), 16),
          g: parseInt(clean.slice(2, 4), 16),
          b: parseInt(clean.slice(4, 6), 16),
        };
      }

      function point(x, y) {
        return { x, y };
      }

      function createSeededRandom(seed) {
        let state = seed >>> 0;
        return () => {
          state = (state + 0x6D2B79F5) >>> 0;
          let value = state;
          value = Math.imul(value ^ (value >>> 15), value | 1);
          value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
          return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
        };
      }
    </script>
  </body>
</html>
`;

const result = await runBrowserPage({
  pluginName: "polygon-shard-renderer-page",
  path: "/__polygon-shard-renderer",
  html,
  waitUntil: "networkidle",
  viewport: { width: 1400, height: 920 },
  deviceScaleFactor: 1,
}, (page) => waitForPageResult(page, "__polygonShardRender", 15_000));

await writeDataUrlPngMap(outputDir, result);
console.log(outputDir);
