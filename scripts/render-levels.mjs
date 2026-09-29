import { writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { repoRoot, runBrowserPage, writeDataUrlPng } from "./benchmark-browser-harness.mjs";

const outputDir = path.resolve(repoRoot, process.argv[2] ?? "artifacts/level-renders");

const html = String.raw`
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>Level Renderer</title>
    <style>
      body {
        margin: 0;
        background: #020807;
      }
      canvas {
        display: block;
      }
    </style>
  </head>
  <body>
    <canvas id="level-render"></canvas>
    <script type="module">
      import levelsJson from "/game-levels.json";
      import { DESKTOP_GAME_PROFILE, MOBILE_GAME_PROFILE, GameMode } from "/src/game-profile.ts";
      import { canPlaceTower } from "/src/placement-rules.ts";
      import { createRouteMotionPath } from "/src/route-path.ts";

      const ROAD_COLOR = "rgba(8, 40, 36, 0.96)";
      const ROAD_BORDER_COLOR = "rgb(18, 61, 54)";
      const BLOCKED_TOWER_COLOR = "rgba(255, 126, 126, 0.38)";
      const EXIT_MARKER_RADIUS = 18;

      const canvas = document.getElementById("level-render");
      const context = canvas.getContext("2d");

      window.__levelRender = {
        render(mode) {
          const profile = mode === GameMode.Mobile ? MOBILE_GAME_PROFILE : DESKTOP_GAME_PROFILE;
          const levels = normalizeLevels(levelsJson, mode);
          const cellPadding = mode === GameMode.Mobile ? 28 : 34;
          const titleHeight = mode === GameMode.Mobile ? 56 : 58;
          const footerHeight = mode === GameMode.Mobile ? 42 : 40;
          const cellWidth = profile.fieldWidth + (cellPadding * 2);
          const cellHeight = profile.fieldHeight + titleHeight + footerHeight + (cellPadding * 2);
          const columns = mode === GameMode.Mobile ? 2 : 2;
          const rows = Math.ceil(levels.length / columns);
          const width = columns * cellWidth;
          const height = rows * cellHeight;

          canvas.width = width * 2;
          canvas.height = height * 2;
          canvas.style.width = width + "px";
          canvas.style.height = height + "px";
          context.setTransform(2, 0, 0, 2, 0, 0);
          context.fillStyle = "#020807";
          context.fillRect(0, 0, width, height);

          const metrics = [];
          for (const [index, level] of levels.entries()) {
            const column = index % columns;
            const row = Math.floor(index / columns);
            const x = column * cellWidth;
            const y = row * cellHeight;
            metrics.push(drawLevel(context, level, profile, index, x, y, cellWidth, cellHeight, cellPadding, titleHeight, footerHeight));
          }

          return {
            dataUrl: canvas.toDataURL("image/png"),
            metrics,
          };
        },
      };

      function normalizeLevels(data, mode) {
        return data.map((level) => {
          const normalized = mode !== GameMode.Mobile || !level.mobile
            ? { ...level }
            : { ...level, ...level.mobile };
          delete normalized.mobile;
          return {
            ...normalized,
            points: normalized.points.map(normalizeLevelPoint),
          };
        });
      }

      function normalizeLevelPoint(point) {
        const [x, y] = point;
        return { x, y };
      }

      function drawLevel(context, level, profile, index, x, y, cellWidth, cellHeight, cellPadding, titleHeight, footerHeight) {
        const fieldX = x + cellPadding;
        const fieldY = y + titleHeight + cellPadding;
        const routePath = createRouteMotionPath(level.points, profile.roadTurnRadius, profile.routeCurveSampleStep);
        const placementMask = createPlacementMask(routePath, profile);
        const pathLength = routePath.entries[routePath.entries.length - 1]?.totalDistance ?? 0;

        drawCardBackground(context, x, y, cellWidth, cellHeight);
        drawTitle(context, level, index, x, y, cellWidth, titleHeight);

        context.save();
        context.translate(fieldX, fieldY);
        drawField(context, profile);
        drawBlockedPlacementMask(context, placementMask);
        drawRoute(context, routePath, level, profile);
        drawTurnCoordinates(context, level);
        context.restore();

        drawFooter(context, level, profile, placementMask.coverage, pathLength, x, fieldY + profile.fieldHeight, cellWidth, footerHeight);

        return {
          level: index + 1,
          name: level.name,
          placeableCoverage: placementMask.coverage,
          pathLength: Math.round(pathLength),
          pointCount: level.points.length,
        };
      }

      function drawCardBackground(context, x, y, width, height) {
        context.save();
        context.fillStyle = "#020807";
        context.fillRect(x, y, width, height);
        context.strokeStyle = "rgba(255, 255, 255, 0.08)";
        context.strokeRect(x + 0.5, y + 0.5, width - 1, height - 1);
        context.restore();
      }

      function drawTitle(context, level, index, x, y, width, titleHeight) {
        context.save();
        context.fillStyle = "rgba(239, 255, 247, 0.95)";
        context.font = "900 20px Avenir Next, Arial Black, Trebuchet MS, system-ui, sans-serif";
        context.textAlign = "left";
        context.textBaseline = "middle";
        context.fillText(String(index + 1).padStart(2, "0") + " " + level.name, x + 22, y + (titleHeight / 2));
        context.restore();
      }

      function drawFooter(context, level, profile, placementCoverage, pathLength, x, y, width, footerHeight) {
        context.save();
        context.fillStyle = "rgba(239, 255, 247, 0.72)";
        context.font = "700 13px Inter, system-ui, sans-serif";
        context.textAlign = "center";
        context.textBaseline = "middle";
        const text = profile.mode + "  |  " + Math.round(placementCoverage * 100) + "% placeable  |  route " + Math.round(pathLength) + "px  |  $" + level.startingMoney;
        context.fillText(text, x + (width / 2), y + (footerHeight / 2));
        context.restore();
      }

      function drawField(context, profile) {
        const gradient = context.createLinearGradient(0, 0, 0, profile.fieldHeight);
        gradient.addColorStop(0, "#010302");
        gradient.addColorStop(0.5, "#050d0a");
        gradient.addColorStop(1, "#010302");
        context.fillStyle = gradient;
        context.fillRect(0, 0, profile.fieldWidth, profile.fieldHeight);

        context.save();
        context.strokeStyle = "rgba(255, 255, 255, 0.06)";
        context.lineWidth = 1;
        for (let gridX = 0; gridX <= profile.fieldWidth; gridX += 35) {
          context.beginPath();
          context.moveTo(gridX, 0);
          context.lineTo(gridX, profile.fieldHeight);
          context.stroke();
        }
        for (let gridY = 0; gridY <= profile.fieldHeight; gridY += 35) {
          context.beginPath();
          context.moveTo(0, gridY);
          context.lineTo(profile.fieldWidth, gridY);
          context.stroke();
        }
        context.restore();
      }

      function drawBlockedPlacementMask(context, placementMask) {
        context.save();
        context.fillStyle = BLOCKED_TOWER_COLOR;
        for (const cell of placementMask.blockedCells) {
          context.fillRect(cell.x, cell.y, cell.size, cell.size);
        }
        context.restore();
      }

      function drawRoute(context, routePath, level, profile) {
        const last = level.points[level.points.length - 1];
        context.save();
        context.lineJoin = "round";
        context.lineCap = "round";

        context.strokeStyle = ROAD_BORDER_COLOR;
        context.lineWidth = profile.roadWidth + 3;
        traceRoutePath(context, routePath);
        context.stroke();

        context.fillStyle = ROAD_BORDER_COLOR;
        context.beginPath();
        context.arc(last.x, last.y, EXIT_MARKER_RADIUS + 1.5, 0, Math.PI * 2);
        context.fill();

        context.strokeStyle = ROAD_COLOR;
        context.lineWidth = profile.roadWidth;
        traceRoutePath(context, routePath);
        context.stroke();

        context.fillStyle = ROAD_COLOR;
        context.beginPath();
        context.arc(last.x, last.y, EXIT_MARKER_RADIUS, 0, Math.PI * 2);
        context.fill();

        context.fillStyle = "rgba(238, 255, 248, 0.86)";
        context.font = "700 15px Inter, system-ui, sans-serif";
        context.textAlign = "center";
        context.textBaseline = "middle";
        context.fillText(String(level.allowEscape), last.x, last.y + 1);
        context.restore();
      }

      function drawTurnCoordinates(context, level) {
        context.save();
        context.font = "800 10px Inter, system-ui, sans-serif";
        context.textAlign = "center";
        context.textBaseline = "middle";
        for (let index = 0; index < level.points.length; index += 1) {
          const point = level.points[index];
          const label = (index + 1) + ": " + Math.round(point.x) + "," + Math.round(point.y);
          const metrics = context.measureText(label);
          const labelWidth = Math.ceil(metrics.width) + 7;
          const labelHeight = 13;
          context.fillStyle = "rgba(1, 8, 7, 0.82)";
          context.fillRect(point.x - (labelWidth / 2), point.y - (labelHeight / 2), labelWidth, labelHeight);
          context.strokeStyle = "rgba(239, 255, 247, 0.55)";
          context.lineWidth = 0.75;
          context.strokeRect(point.x - (labelWidth / 2), point.y - (labelHeight / 2), labelWidth, labelHeight);
          context.fillStyle = "rgba(239, 255, 247, 0.96)";
          context.fillText(label, point.x, point.y + 0.5);
        }
        context.restore();
      }

      function traceRoutePath(context, routePath) {
        context.beginPath();
        context.moveTo(routePath.start.x, routePath.start.y);
        for (const command of routePath.commands) {
          if (command.kind === "line") {
            context.lineTo(command.point.x, command.point.y);
          } else {
            context.quadraticCurveTo(command.control.x, command.control.y, command.point.x, command.point.y);
          }
        }
      }

      function createPlacementMask(routePath, profile) {
        const blockedCells = [];
        const sampleSize = 3;
        const halfSample = sampleSize / 2;
        let validCells = 0;
        let sampledCells = 0;
        for (let y = 0; y < profile.fieldHeight; y += sampleSize) {
          for (let x = 0; x < profile.fieldWidth; x += sampleSize) {
            sampledCells += 1;
            const point = {
              x: x + halfSample,
              y: y + halfSample,
            };
            if (!canPlaceTower(point, routePath, [], profile.placement)) {
              blockedCells.push({ x, y, size: sampleSize });
              continue;
            }
            validCells += 1;
          }
        }
        return {
          blockedCells,
          coverage: sampledCells === 0 ? 0 : validCells / sampledCells,
        };
      }
    </script>
  </body>
</html>
`;

const renderedLevels = await runBrowserPage({
  pluginName: "level-renderer-page",
  path: "/__level-renderer",
  html,
  waitUntil: "networkidle",
  viewport: { width: 1200, height: 900 },
  deviceScaleFactor: 1,
}, async (page) => {
  const results = [];
  for (const mode of ["desktop", "mobile"]) {
    const result = await page.evaluate((renderMode) => window.__levelRender.render(renderMode), mode);
    results.push({ mode, result });
  }
  return results;
});

const summaries = [];
for (const { mode, result } of renderedLevels) {
  const outputPath = path.join(outputDir, `${mode}.png`);
  await writeDataUrlPng(outputPath, result.dataUrl);
  summaries.push({ mode, outputPath, metrics: result.metrics });
}

const summaryPath = path.join(outputDir, "summary.json");
await writeFile(summaryPath, `${JSON.stringify(summaries, null, 2)}\n`);
console.log(summaryPath);
for (const summary of summaries) {
  console.log(summary.outputPath);
}
