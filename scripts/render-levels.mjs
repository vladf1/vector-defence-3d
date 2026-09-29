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
      import init, { levelSheet } from "/src/generated/engine-labs/engine.js";

      await init();

      const ROAD_COLOR = "rgba(8, 40, 36, 0.96)";
      const ROAD_BORDER_COLOR = "rgb(18, 61, 54)";
      const BLOCKED_TOWER_COLOR = "rgba(255, 126, 126, 0.38)";
      const EXIT_MARKER_RADIUS = 18;

      const canvas = document.getElementById("level-render");
      const context = canvas.getContext("2d");

      window.__levelRender = {
        render(mode) {
          const sheet = JSON.parse(levelSheet(mode === "mobile"));
          const profile = { mode, fieldWidth: sheet.fieldWidth, fieldHeight: sheet.fieldHeight, roadWidth: sheet.roadWidth };
          const levels = sheet.levels.map((level) => ({ ...level, points: level.points.map(([x, y]) => ({ x, y })) }));
          const cellPadding = mode === "mobile" ? 28 : 34;
          const titleHeight = mode === "mobile" ? 56 : 58;
          const footerHeight = mode === "mobile" ? 42 : 40;
          const cellWidth = profile.fieldWidth + (cellPadding * 2);
          const cellHeight = profile.fieldHeight + titleHeight + footerHeight + (cellPadding * 2);
          const columns = 2;
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
            metrics.push(drawLevel(context, level, profile, sheet.sample, index, column * cellWidth, row * cellHeight, cellWidth, cellHeight, cellPadding, titleHeight, footerHeight));
          }

          return {
            dataUrl: canvas.toDataURL("image/png"),
            metrics,
          };
        },
      };

      function drawLevel(context, level, profile, sample, index, x, y, cellWidth, cellHeight, cellPadding, titleHeight, footerHeight) {
        const fieldX = x + cellPadding;
        const fieldY = y + titleHeight + cellPadding;
        const routePath = level;
        const placementMask = createPlacementMask(level.blocked, profile, sample);
        const pathLength = level.pathLength;

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
        context.moveTo(routePath.start[0], routePath.start[1]);
        for (const command of routePath.commands) {
          if (command.length === 2) {
            context.lineTo(command[0], command[1]);
          } else {
            context.quadraticCurveTo(command[0], command[1], command[2], command[3]);
          }
        }
      }

      // The engine samples placement on a grid (`levelSheet`); 1 marks a blocked cell.
      function createPlacementMask(blocked, profile, sampleSize) {
        const blockedCells = [];
        const columns = Math.ceil(profile.fieldWidth / sampleSize);
        let validCells = 0;
        for (let index = 0; index < blocked.length; index += 1) {
          if (blocked[index] === "1") {
            blockedCells.push({ x: (index % columns) * sampleSize, y: Math.floor(index / columns) * sampleSize, size: sampleSize });
          } else {
            validCells += 1;
          }
        }
        return {
          blockedCells,
          coverage: blocked.length === 0 ? 0 : validCells / blocked.length,
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
