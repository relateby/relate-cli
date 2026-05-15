// gram render.js — Paper.js + d3-force graph visualization
// Reads gram-data JSON, runs layout verification, and draws the graph.
// Arc geometry adapted from Neo4j Browser 4.0.10 (pairwiseArcsRelationshipRouting / arcArrow).

(function () {
  // Parse embedded graph data
  var dataEl = document.getElementById("gram-data");
  var data = JSON.parse(dataEl.textContent);
  var nodes = data.nodes || [];
  var edges = data.edges || [];
  var paths = data.paths || [];
  var layout = data.layout || {};
  var positions = layout.positions || {};

  // Index nodes by id
  var nodeById = {};
  nodes.forEach(function (n) { nodeById[n.id] = n; });

  // Fix canvas to the actual viewport size before Paper.js reads it.
  // Without this, Paper.js inflates canvas.style.width (often to 2× window height),
  // pushing the sidebar off-screen and breaking the coordinate system.
  var canvas = document.getElementById("gram-canvas");
  var sidebarEl = document.getElementById("sidebar");
  var sidebarW = sidebarEl ? sidebarEl.offsetWidth : 0;
  var canvasW  = window.innerWidth - sidebarW;
  var canvasH  = window.innerHeight;
  canvas.width  = Math.round(canvasW * window.devicePixelRatio);
  canvas.height = Math.round(canvasH * window.devicePixelRatio);
  canvas.style.width  = canvasW + "px";
  canvas.style.height = canvasH + "px";

  // Set up Paper.js canvas
  paper.setup(canvas);
  var view = paper.view;

  var NODE_RADIUS = 22;
  var SHAFT_WIDTH = 2;
  var HEAD_WIDTH = SHAFT_WIDTH + 6;   // 8
  var HEAD_HEIGHT = HEAD_WIDTH;       // 8 — same proportions as Neo4j Browser
  var DEFLECTION_STEP = 30;           // degrees per parallel-edge step
  var MAX_DEFLECTION = 150;           // degrees total spread cap
  var SAGITTA_PER_DEG = 1.5;         // pixels of arc height per degree of deflection

  // ── Path envelope layer ────────────────────────────────────────────────────────
  var PATH_COLORS = [
    "hsla(200,70%,60%,0.18)", "hsla(120,60%,55%,0.18)",
    "hsla(40,80%,60%,0.18)", "hsla(300,60%,65%,0.18)",
    "hsla(0,70%,60%,0.18)", "hsla(160,60%,55%,0.18)"
  ];

  paths.forEach(function (path, colorIdx) {
    var memberIds = [];
    (path.members || []).forEach(function (m) {
      if (m.kind === "Node") memberIds.push(m.id);
    });
    if (memberIds.length < 2) return;

    var pts = memberIds.map(function (id) {
      var p = positions[id];
      return p ? new paper.Point(p.x, p.y) : null;
    }).filter(Boolean);
    if (pts.length < 2) return;

    var hull = convexHull(pts);
    if (hull.length < 2) return;

    var cx = hull.reduce(function (s, p) { return s + p.x; }, 0) / hull.length;
    var cy = hull.reduce(function (s, p) { return s + p.y; }, 0) / hull.length;
    var PAD = 32;
    var expanded = hull.map(function (p) {
      var dx = p.x - cx, dy = p.y - cy;
      var len = Math.sqrt(dx * dx + dy * dy) || 1;
      return new paper.Point(cx + (len + PAD) * dx / len, cy + (len + PAD) * dy / len);
    });

    var shape = new paper.Path({ closed: true });
    expanded.forEach(function (p, i) {
      var next = expanded[(i + 1) % expanded.length];
      var mid = p.add(next).divide(2);
      if (i === 0) shape.moveTo(mid);
      else shape.quadraticCurveTo(p, mid);
    });
    shape.fillColor = PATH_COLORS[colorIdx % PATH_COLORS.length];
    shape.strokeColor = PATH_COLORS[colorIdx % PATH_COLORS.length].replace("0.18", "0.5");
    shape.strokeWidth = 1.5;

    if (path.id) {
      new paper.PointText({
        point: new paper.Point(cx, cy - PAD - 10),
        content: path.id,
        fontSize: 11,
        fillColor: "#666",
        justification: "center"
      });
    }
  });

  // ── Pairwise arc routing ───────────────────────────────────────────────────────
  // Group edges by unordered node pair, then assign deflection angles.

  var pairMap = {}; // key "a|b" (sorted) → [{edge, flipped}]
  edges.forEach(function (edge) {
    var a = edge.source, b = edge.target;
    var key = a < b ? a + "|" + b : b + "|" + a;
    if (!pairMap[key]) pairMap[key] = { nodeA: a < b ? a : b, nodeB: a < b ? b : a, items: [] };
    pairMap[key].items.push(edge);
  });

  // Draw each pair with proper deflection
  Object.keys(pairMap).forEach(function (key) {
    var pair = pairMap[key];
    var items = pair.items;
    var count = items.length;

    var middleIdx = (count - 1) / 2;
    var totalDeflection = DEFLECTION_STEP * (count - 1);
    var step = totalDeflection > MAX_DEFLECTION
      ? MAX_DEFLECTION / (count - 1)
      : DEFLECTION_STEP;

    items.forEach(function (edge, i) {
      var deflection = step * (i - middleIdx);
      // Flip sign when edge runs opposite to the canonical pair direction
      if (pair.nodeA !== edge.source) deflection = -deflection;
      drawEdge(edge, deflection);
    });
  });

  // ── Edge drawing ───────────────────────────────────────────────────────────────

  function drawEdge(edge, deflection) {
    var sp = positions[edge.source];
    var tp = positions[edge.target];
    if (!sp || !tp) return;

    var dx = tp.x - sp.x, dy = tp.y - sp.y;
    var centreDistance = Math.sqrt(dx * dx + dy * dy) || 1;
    var naturalAngle = Math.atan2(dy, dx); // radians, source→target

    var shaftR = SHAFT_WIDTH / 2;
    var headR = HEAD_WIDTH / 2;

    if (Math.abs(deflection) < 0.5) {
      drawStraightArrow(sp, tp, naturalAngle, centreDistance, edge, shaftR, headR);
    } else {
      drawArcArrow(sp, tp, naturalAngle, centreDistance, edge, deflection, shaftR, headR);
    }
  }

  // Straight arrow (no deflection) — simple line + filled triangle arrowhead
  function drawStraightArrow(sp, tp, angle, dist, edge, shaftR, headR) {
    var cos = Math.cos(angle), sin = Math.sin(angle);

    var x1 = sp.x + cos * NODE_RADIUS;
    var y1 = sp.y + sin * NODE_RADIUS;
    var tipX = tp.x - cos * NODE_RADIUS;
    var tipY = tp.y - sin * NODE_RADIUS;
    var shaftEndX = tipX - cos * HEAD_HEIGHT;
    var shaftEndY = tipY - sin * HEAD_HEIGHT;

    // Perpendicular
    var px = -sin, py = cos;

    var pathData = [
      "M", x1 + px * shaftR,           y1 + py * shaftR,
      "L", shaftEndX + px * shaftR,    shaftEndY + py * shaftR,
      "L", shaftEndX + px * headR,     shaftEndY + py * headR,
      "L", tipX,                        tipY,
      "L", shaftEndX - px * headR,     shaftEndY - py * headR,
      "L", shaftEndX - px * shaftR,    shaftEndY - py * shaftR,
      "L", x1 - px * shaftR,           y1 - py * shaftR,
      "Z"
    ].join(" ");

    new paper.Path({
      pathData: pathData,
      fillColor: edge.directed !== false ? "#aaa" : null,
      strokeColor: "#aaa",
      strokeWidth: edge.directed !== false ? 0 : SHAFT_WIDTH,
    });

    drawEdgeLabel(edge, (x1 + shaftEndX) / 2, (y1 + shaftEndY) / 2, angle);
  }

  // Arc arrow — true circular arc via sagitta geometry.
  // Chord spans center-to-center so the arc, if extended, passes through each node center.
  // Attachment points T1/T2 are where the arc circle intersects each node circle.
  function drawArcArrow(sp, tp, naturalAngle, centreDistance, edge, deflectionDeg, shaftR, headR) {
    var sagitta = deflectionDeg * SAGITTA_PER_DEG;
    var absS = Math.abs(sagitta);
    if (absS < 0.5) {
      drawStraightArrow(sp, tp, naturalAngle, centreDistance, edge, shaftR, headR);
      return;
    }

    // Perpendicular unit vector (left of chord direction)
    var perpX = -Math.sin(naturalAngle), perpY = Math.cos(naturalAngle);

    // Chord = center-to-center; arc passes through both node centers when extended
    var halfChord = centreDistance / 2;
    var R = (halfChord * halfChord + absS * absS) / (2 * absS);

    // Arc center: placed opposite to the arc peak, at distance (R - s) from midpoint
    var sign = sagitta > 0 ? 1 : -1;
    var midX = (sp.x + tp.x) / 2, midY = (sp.y + tp.y) / 2;
    var arcCx = midX - sign * perpX * (R - absS);
    var arcCy = midY - sign * perpY * (R - absS);

    // Angles from arc center to node centers (sp and tp lie on the arc circle)
    var alphaS = Math.atan2(sp.y - arcCy, sp.x - arcCx);
    var alphaT = Math.atan2(tp.y - arcCy, tp.x - arcCx);

    // Angular step to move NODE_RADIUS along the arc from each node center.
    // Exact: delta = acos(1 - r²/(2R²)) where r = NODE_RADIUS.
    var delta = Math.acos(Math.max(-1, Math.min(1, 1 - (NODE_RADIUS * NODE_RADIUS) / (2 * R * R))));

    // Determine which direction the arc travels (CW vs CCW in screen coords)
    var rawSweep = alphaT - alphaS;
    while (rawSweep >  Math.PI) rawSweep -= 2 * Math.PI;
    while (rawSweep < -Math.PI) rawSweep += 2 * Math.PI;
    if (sign > 0 && rawSweep > 0) rawSweep -= 2 * Math.PI;
    if (sign < 0 && rawSweep < 0) rawSweep += 2 * Math.PI;
    var sweepDir = rawSweep > 0 ? 1 : -1;

    // T1: arc exits the source node circle; T2: arc enters the target node circle
    var t1Angle = alphaS + sweepDir * delta;
    var t2Angle = alphaT - sweepDir * delta;
    var t1x = arcCx + R * Math.cos(t1Angle), t1y = arcCy + R * Math.sin(t1Angle);
    var t2x = arcCx + R * Math.cos(t2Angle), t2y = arcCy + R * Math.sin(t2Angle);

    // Arc sweep from t1 to t2
    var sweep = t2Angle - t1Angle;
    if (sweepDir > 0) { while (sweep < 0) sweep += 2 * Math.PI; }
    else              { while (sweep > 0) sweep -= 2 * Math.PI; }
    var sweepFlag = sweepDir > 0 ? 1 : 0;
    var largeArc  = Math.abs(sweep) > Math.PI ? 1 : 0;

    // Tangent direction at T2 for arrowhead
    var cosE = Math.cos(t2Angle), sinE = Math.sin(t2Angle);
    var tx = sweepFlag === 1 ?  sinE : -sinE;
    var ty = sweepFlag === 1 ? -cosE :  cosE;
    var px = cosE, py = sinE;

    // Step back from T2 by HEAD_HEIGHT to get shaft end
    var stepAngle = HEAD_HEIGHT / R;
    var shaftEndAngle = sweepFlag === 1 ? t2Angle - stepAngle : t2Angle + stepAngle;
    var cosSHA = Math.cos(shaftEndAngle), sinSHA = Math.sin(shaftEndAngle);

    // Parallel arc endpoints at shaftR offset
    var rOuter = R + shaftR, rInner = R - shaftR;
    var outerStartX = arcCx + rOuter * Math.cos(t1Angle);
    var outerStartY = arcCy + rOuter * Math.sin(t1Angle);
    var innerStartX = arcCx + rInner * Math.cos(t1Angle);
    var innerStartY = arcCy + rInner * Math.sin(t1Angle);
    var outerEndX   = arcCx + rOuter * cosSHA;
    var outerEndY   = arcCy + rOuter * sinSHA;
    var innerEndX   = arcCx + rInner * cosSHA;
    var innerEndY   = arcCy + rInner * sinSHA;

    // Filled path: outer arc → arrowhead → inner arc reversed → close
    var pathData = [
      "M", outerStartX, outerStartY,
      "A", rOuter, rOuter, 0, largeArc, sweepFlag,     outerEndX, outerEndY,
      "L", outerEndX + (px - tx) * (headR - shaftR), outerEndY + (py - ty) * (headR - shaftR),
      "L", t2x, t2y,
      "L", innerEndX - (px + tx) * (headR - shaftR), innerEndY - (py + ty) * (headR - shaftR),
      "L", innerEndX, innerEndY,
      "A", rInner, rInner, 0, largeArc, 1 - sweepFlag, innerStartX, innerStartY,
      "Z"
    ].join(" ");

    new paper.Path({ pathData: pathData, fillColor: "#aaa" });

    // Label at arc midpoint
    var midAngle = t1Angle + sweep * 0.5;
    var midX = arcCx + R * Math.cos(midAngle);
    var midY = arcCy + R * Math.sin(midAngle);
    var lDeg = naturalAngle * 180 / Math.PI;
    if (lDeg > 90 || lDeg < -90) lDeg += 180;
    drawEdgeLabelAt(edge, midX, midY, lDeg);
  }

  function drawEdgeLabel(edge, mx, my, angle) {
    if (!edge.label) return;
    var deg = angle * 180 / Math.PI;
    if (deg > 90 || deg < -90) deg += 180; // keep text right-side up
    drawEdgeLabelAt(edge, mx, my, deg);
  }

  function drawEdgeLabelAt(edge, x, y, deg) {
    if (!edge.label) return;
    var text = new paper.PointText({
      point: new paper.Point(x, y),
      content: edge.label,
      fontSize: 10,
      fillColor: "#888",
      justification: "center"
    });
    text.rotate(deg, new paper.Point(x, y));
  }

  // ── Draw nodes ─────────────────────────────────────────────────────────────────
  var nodeGroups = {};
  nodes.forEach(function (node) {
    var p = positions[node.id];
    if (!p) return;
    var center = new paper.Point(p.x, p.y);

    var circle = new paper.Path.Circle({
      center: center,
      radius: NODE_RADIUS,
      fillColor: node.is_nested ? "#e8e8f8" : "#e8f4fb",
      strokeColor: node.is_nested ? "#9999cc" : "#3399cc",
      strokeWidth: 1.5
    });

    var labelText = node.id;
    if (node.labels && node.labels.length > 0) {
      labelText += ":" + node.labels[0];
    }
    var label = new paper.PointText({
      point: new paper.Point(p.x, p.y + 4),
      content: labelText,
      fontSize: 10,
      fillColor: "#333",
      justification: "center"
    });

    var group = new paper.Group([circle, label]);
    group.data = node;
    nodeGroups[node.id] = group;
  });

  // ── Click-to-inspect ───────────────────────────────────────────────────────────
  var sidebar = document.getElementById("sidebar");
  paper.view.onMouseDown = function (event) {
    var hit = paper.project.hitTest(event.point, { fill: true, stroke: true, tolerance: 5 });
    if (!hit) return;
    var item = hit.item;
    while (item && Object.keys(item.data || {}).length === 0) item = item.parent;
    if (item && item.data) {
      sidebar.textContent = JSON.stringify(item.data, null, 2);
    }
  };

  // ── Pan (space+drag) / Zoom (Ctrl/Cmd+scroll) ─────────────────────────────────
  paper.view.onMouseDrag = function (event) {
    if (event.modifiers.space) {
      paper.view.translate(event.delta);
    }
  };
  paper.view.element.addEventListener("wheel", function (e) {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    var factor = e.deltaY > 0 ? 0.9 : 1.1;
    var pt = paper.view.viewToProject(new paper.Point(e.offsetX, e.offsetY));
    paper.view.scale(factor, pt);
  }, { passive: false });

  // ── Center and fit graph in visible canvas area ────────────────────────────────
  // Paper.js inflates canvas.style.width via setup(); use window dimensions instead.
  var graphBounds = paper.project.activeLayer.bounds;
  if (graphBounds && graphBounds.width > 0) {
    var PADDING = NODE_RADIUS * 2;
    var sidebarEl = document.getElementById("sidebar");
    var sidebarW = sidebarEl ? sidebarEl.offsetWidth : 0;
    var vw = window.innerWidth - sidebarW;
    var vh = window.innerHeight;
    var fitSx = (vw - PADDING * 2) / graphBounds.width;
    var fitSy = (vh - PADDING * 2) / graphBounds.height;
    var fitScale = Math.min(fitSx, fitSy, 1);
    var cx = vw / 2, cy = vh / 2;
    paper.project.activeLayer.translate(
      new paper.Point(cx - graphBounds.center.x, cy - graphBounds.center.y)
    );
    if (fitScale < 1) {
      paper.project.activeLayer.scale(fitScale, new paper.Point(cx, cy));
    }
  }

  paper.view.draw();

  // ── Convex hull utility (gift-wrapping) ───────────────────────────────────────
  function convexHull(points) {
    if (points.length < 3) return points;
    var n = points.length;
    var start = 0;
    for (var i = 1; i < n; i++) {
      if (points[i].x < points[start].x) start = i;
    }
    var hull = [];
    var p = start;
    do {
      hull.push(points[p]);
      var q = (p + 1) % n;
      for (var i = 0; i < n; i++) {
        if (cross(points[p], points[q], points[i]) < 0) q = i;
      }
      p = q;
    } while (p !== start);
    return hull;
  }

  function cross(o, a, b) {
    return (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
  }
})();
