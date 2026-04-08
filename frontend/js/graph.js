const LAYER_COLORS = {
  WET: { bg: '#dbeafe', border: '#3b82f6', text: '#1e40af' },
  AMVB: { bg: '#dcfce7', border: '#22c55e', text: '#166534' },
  MINISTERIELE_REGELING: { bg: '#fce7f3', border: '#ec4899', text: '#9d174d' },
  GEMEENTELIJKE_VERORDENING: { bg: '#fef3c7', border: '#f59e0b', text: '#92400e' },
  ONBEKEND: { bg: '#f1f5f9', border: '#94a3b8', text: '#475569' },
};

const VALIDATION_STATUS = {
  none:        { border: null, bg: null, label: 'Geen status' },
  pending:     { border: '#ea580c', bg: '#fff7ed', label: 'Te controleren' },
  in_progress: { border: '#ca8a04', bg: '#fefce8', label: 'Bezig' },
  validated:   { border: '#16a34a', bg: '#f0fdf4', label: 'Gevalideerd' },
};
const STATUS_CYCLE = ['none', 'pending', 'in_progress', 'validated'];
const nodeStatusMap = new Map();

function getLayerColor(layer) {
  return LAYER_COLORS[layer] || LAYER_COLORS.ONBEKEND;
}

function formatLawName(name) {
  if (name.startsWith('#')) return name.substring(1).replace(/_/g, ' ');
  return name.replace(/_/g, ' ');
}

function formatLayerLabel(layer) {
  const labels = {
    WET: 'Wet',
    AMVB: 'AMvB',
    MINISTERIELE_REGELING: 'Min. regeling',
    GEMEENTELIJKE_VERORDENING: 'Verordening',
  };
  return labels[layer] || layer;
}

let cy = null;
let graphData = null;
let selectedNode = null;
let parsedScenarios = [];
let activeScenarioIdx = null;

async function init() {
  try {
    const resp = await fetch('/data/graph-data.json');
    if (resp.ok) {
      graphData = await resp.json();
    }
  } catch (e) {
    // No pre-generated data — that's fine, user can upload YAML
  }

  if (!graphData) {
    document.getElementById('graph-canvas').innerHTML =
      '<p style="padding:2rem;color:#64748b;">Geen graaf-data gevonden. Upload YAML-bestanden of een map via de sidebar.</p>';
  }

  try {
    const featResp = await fetch('/data/features-data.json');
    if (featResp.ok) {
      const featuresData = await featResp.json();
      parsedScenarios = parseAllScenarios(featuresData);
    }
  } catch (e) {
    console.warn('Features data not available');
  }

  if (graphData) {
    buildScenarioFilters();
    buildFilters();
    renderGraph(graphData);
  }
}

// Compute connected component from seed law IDs
function getConnectedLaws(data, seedIds) {
  const connected = new Set(seedIds);
  let changed = true;
  while (changed) {
    changed = false;
    for (const edge of data.edges) {
      if (connected.has(edge.source) && !connected.has(edge.target)) {
        connected.add(edge.target);
        changed = true;
      }
      if (connected.has(edge.target) && !connected.has(edge.source)) {
        connected.add(edge.source);
        changed = true;
      }
    }
  }
  return connected;
}

function setAllCheckboxes(checked) {
  document.querySelectorAll('#law-filters input[type="checkbox"]').forEach(cb => {
    cb.checked = checked;
  });
}

function buildFilters() {
  const container = document.getElementById('law-filters');
  if (!container || !graphData) return;

  container.innerHTML = '';

  // Only show laws that are visible in the graph
  const connectedIds = new Set();
  for (const edge of graphData.edges) {
    connectedIds.add(edge.source);
    connectedIds.add(edge.target);
  }
  const visibleLaws = graphData.laws.filter(law =>
    (law.articles && law.articles.length > 0) || connectedIds.has(law.id)
  );

  const groups = {};
  for (const law of visibleLaws) {
    const layer = law.regulatory_layer || 'ONBEKEND';
    if (!groups[layer]) groups[layer] = [];
    groups[layer].push(law);
  }

  const layerOrder = ['WET', 'AMVB', 'MINISTERIELE_REGELING', 'GEMEENTELIJKE_VERORDENING', 'ONBEKEND'];

  for (const layer of layerOrder) {
    const laws = groups[layer];
    if (!laws || laws.length === 0) continue;

    const section = document.createElement('div');
    section.className = 'graph-sidebar__section';

    const sectionTitle = document.createElement('div');
    sectionTitle.className = 'graph-sidebar__section-title';
    sectionTitle.textContent = formatLayerLabel(layer);
    section.appendChild(sectionTitle);

    for (const law of laws) {
      const label = document.createElement('label');
      label.className = 'graph-sidebar__checkbox';

      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.checked = true;
      checkbox.value = law.id;
      checkbox.addEventListener('change', () => updateVisibility());

      const span = document.createElement('span');
      span.textContent = formatLawName(law.name);
      span.title = law.id;

      label.appendChild(checkbox);
      label.appendChild(span);
      section.appendChild(label);
    }

    container.appendChild(section);
  }
}

function buildArticleLabel(art) {
  const lines = [`Art. ${art.number}`];
  if (art.inputs.length > 0 || art.outputs.length > 0) {
    lines.push('\u2500'.repeat(16));
  }
  for (const inp of art.inputs) {
    lines.push(`\u2192 ${inp.name}`);
  }
  if (art.inputs.length > 0 && art.outputs.length > 0) {
    lines.push('\u2500'.repeat(16));
  }
  for (const out of art.outputs) {
    lines.push(`\u2190 ${out.name}`);
  }
  return lines.join('\n');
}

function calcArticleSize(art) {
  const lineCount = 1 + art.inputs.length + art.outputs.length +
    (art.inputs.length > 0 || art.outputs.length > 0 ? 1 : 0) +
    (art.inputs.length > 0 && art.outputs.length > 0 ? 1 : 0);
  const height = Math.max(40, lineCount * 16 + 16);

  let maxLen = `Art. ${art.number}`.length;
  for (const inp of art.inputs) {
    maxLen = Math.max(maxLen, inp.name.length + 2);
  }
  for (const out of art.outputs) {
    maxLen = Math.max(maxLen, out.name.length + 2);
  }
  const width = Math.max(120, maxLen * 7.5 + 24);

  return { width, height };
}

function renderGraph(data) {
  if (!data) return;

  const elements = [];

  // Determine which laws are connected by edges
  const connectedLawIds = new Set();
  for (const edge of data.edges) {
    connectedLawIds.add(edge.source);
    connectedLawIds.add(edge.target);
  }

  // Filter: only show laws that have articles OR are connected by edges
  const visibleLaws = data.laws.filter(law =>
    (law.articles && law.articles.length > 0) || connectedLawIds.has(law.id)
  );

  // Assign partition indices by regulatory layer for ELK grouping
  const LAYER_ORDER = ['WET', 'AMVB', 'MINISTERIELE_REGELING', 'GEMEENTELIJKE_VERORDENING', 'ONBEKEND'];
  const layerPartition = {};
  LAYER_ORDER.forEach((l, i) => { layerPartition[l] = i; });

  // Law parent nodes (compound containers)
  for (const law of visibleLaws) {
    const color = getLayerColor(law.regulatory_layer);
    const partition = layerPartition[law.regulatory_layer] ?? LAYER_ORDER.length;
    elements.push({
      group: 'nodes',
      data: {
        id: law.id,
        label: formatLawName(law.name),
        nodeType: 'law',
        layer: law.regulatory_layer,
        layerLabel: formatLayerLabel(law.regulatory_layer),
        bgColor: color.bg,
        borderColor: color.border,
        textColor: color.text,
        url: law.url,
        elkPartition: partition,
      },
    });

    // Article child nodes
    if (law.articles) {
      for (const art of law.articles) {
        const size = calcArticleSize(art);
        const artId = `${law.id}:${art.number}`;
        elements.push({
          group: 'nodes',
          data: {
            id: artId,
            parent: law.id,
            label: buildArticleLabel(art),
            nodeType: 'article',
            layer: law.regulatory_layer,
            lawId: law.id,
            articleNumber: art.number,
            bgColor: color.bg,
            borderColor: color.border,
            textColor: color.text,
            inputs: art.inputs,
            outputs: art.outputs,
            nodeWidth: size.width,
            nodeHeight: size.height,
          },
        });
      }
    }
  }

  // Edges — article-level (with fallback to law-level)
  const elementIds = new Set(elements.map(el => el.data.id));
  for (const edge of data.edges) {
    let sourceId = edge.source_article ? `${edge.source}:${edge.source_article}` : edge.source;
    let targetId = edge.target_article ? `${edge.target}:${edge.target_article}` : edge.target;

    // Fallback to law node if article node doesn't exist
    if (!elementIds.has(sourceId)) sourceId = edge.source;
    if (!elementIds.has(targetId)) targetId = edge.target;

    if (!elementIds.has(sourceId) || !elementIds.has(targetId)) continue;

    const isLegalBasis = edge.edge_type === 'legal_basis';
    const edgeId = `${sourceId}->${targetId}:${edge.source_output}`;
    elements.push({
      group: 'edges',
      data: {
        id: edgeId,
        source: sourceId,
        target: targetId,
        label: isLegalBasis ? 'grondslag' : (edge.source_output || ''),
        sourceOutput: edge.source_output || '',
        sourceLaw: edge.source,
        targetLaw: edge.target,
        isIntraLaw: edge.source === edge.target,
        isLegalBasis,
      },
    });
  }

  // User input nodes — leaf inputs without incoming edges
  const coveredInputs = new Set();
  for (const edge of data.edges) {
    if (edge.target_input && edge.target) {
      const artKey = edge.target_article
        ? `${edge.target}:${edge.target_article}:${edge.target_input}`
        : `${edge.target}::${edge.target_input}`;
      coveredInputs.add(artKey);
    }
  }

  for (const law of visibleLaws) {
    if (!law.articles) continue;
    for (const art of law.articles) {
      for (const inp of art.inputs) {
        const key = `${law.id}:${art.number}:${inp.name}`;
        if (coveredInputs.has(key)) continue;

        const artNodeId = `${law.id}:${art.number}`;
        if (!elementIds.has(artNodeId)) continue;

        const labelText = inp.name.replace(/_/g, ' ');
        const nodeWidth = Math.max(80, labelText.length * 6.5 + 20);
        const nodeId = `user:${law.id}:${art.number}:${inp.name}`;

        elements.push({
          group: 'nodes',
          data: {
            id: nodeId,
            label: labelText,
            nodeType: 'user_input',
            lawId: law.id,
            fieldName: inp.name,
            nodeWidth: nodeWidth,
            nodeHeight: 26,
          },
        });

        elements.push({
          group: 'edges',
          data: {
            id: `${nodeId}->${artNodeId}`,
            source: nodeId,
            target: artNodeId,
            label: '',
            sourceOutput: inp.name,
            sourceLaw: law.id,
            targetLaw: law.id,
            isUserInput: true,
          },
        });
      }
    }
  }

  if (cy) cy.destroy();

  cy = cytoscape({
    container: document.getElementById('graph-canvas'),
    elements,
    style: [
      // Law compound parent
      {
        selector: 'node[nodeType="law"]',
        style: {
          label: 'data(label)',
          'text-wrap': 'wrap',
          'text-max-width': '200px',
          'text-valign': 'top',
          'text-halign': 'center',
          'text-margin-y': '8px',
          'font-size': '13px',
          'font-weight': 'bold',
          'font-family': 'RijksoverheidSans, system-ui, sans-serif',
          'background-color': 'data(bgColor)',
          'background-opacity': 0.4,
          'border-width': 2,
          'border-color': 'data(borderColor)',
          color: 'data(textColor)',
          shape: 'round-rectangle',
          'padding': '40px 12px 12px 12px',
          'compound-sizing-wrt-labels': 'include',
        },
      },
      // Article child node
      {
        selector: 'node[nodeType="article"]',
        style: {
          label: 'data(label)',
          'text-wrap': 'wrap',
          'text-max-width': '300px',
          'text-valign': 'center',
          'text-halign': 'center',
          'font-size': '10px',
          'font-family': '"SF Mono", "Fira Code", "Consolas", monospace',
          'background-color': '#ffffff',
          'background-opacity': 0.95,
          'border-width': 1.5,
          'border-color': 'data(borderColor)',
          color: '#1e293b',
          shape: 'round-rectangle',
          width: 'data(nodeWidth)',
          height: 'data(nodeHeight)',
        },
      },
      // Edges
      {
        selector: 'edge',
        style: {
          width: 1.5,
          'line-color': '#94a3b8',
          'target-arrow-color': '#94a3b8',
          'target-arrow-shape': 'triangle',
          'curve-style': 'taxi',
          'taxi-direction': 'rightward',
          'taxi-turn': '60px',
          'arrow-scale': 1,
          'font-size': '9px',
          'text-opacity': 0,
          color: '#64748b',
          'source-endpoint': 'outside-to-node',
          'target-endpoint': 'outside-to-node',
        },
      },
      // Intra-law edges
      {
        selector: 'edge[?isIntraLaw]',
        style: {
          'line-style': 'dashed',
          'line-dash-pattern': [6, 3],
        },
      },
      // Legal basis edges (hierarchy)
      {
        selector: 'edge[?isLegalBasis]',
        style: {
          'line-color': '#3b82f6',
          'target-arrow-color': '#3b82f6',
          'line-style': 'dashed',
          'line-dash-pattern': [8, 4],
          width: 1.5,
          'target-arrow-shape': 'triangle',
        },
      },
      // Highlighted states
      {
        selector: 'node.highlighted',
        style: {
          'border-width': 3,
          'border-color': '#154273',
          'z-index': 10,
        },
      },
      {
        selector: 'edge.inbound',
        style: {
          'line-color': '#ef4444',
          'target-arrow-color': '#ef4444',
          width: 2.5,
          'z-index': 10,
          label: 'data(label)',
          'text-opacity': 1,
          'text-background-color': '#ffffff',
          'text-background-opacity': 0.9,
          'text-background-padding': '2px',
        },
      },
      {
        selector: 'edge.outbound',
        style: {
          'line-color': '#22c55e',
          'target-arrow-color': '#22c55e',
          width: 2.5,
          'z-index': 10,
          label: 'data(label)',
          'text-opacity': 1,
          'text-background-color': '#ffffff',
          'text-background-opacity': 0.9,
          'text-background-padding': '2px',
        },
      },
      {
        selector: 'node.dimmed',
        style: { opacity: 0.2 },
      },
      {
        selector: 'edge.dimmed',
        style: { opacity: 0.08 },
      },
      // Scenario overlay styles
      {
        selector: 'edge.scenario-true',
        style: {
          'line-color': '#22c55e',
          'target-arrow-color': '#22c55e',
          width: 2.5,
          'z-index': 10,
          label: 'data(label)',
          'text-opacity': 1,
          'text-background-color': '#f0fdf4',
          'text-background-opacity': 0.95,
          'text-background-padding': '3px',
          color: '#166534',
          'font-size': '9px',
        },
      },
      {
        selector: 'edge.scenario-false',
        style: {
          'line-color': '#f87171',
          'target-arrow-color': '#f87171',
          width: 1.5,
          'z-index': 5,
          label: 'data(label)',
          'text-opacity': 1,
          'text-background-color': '#fef2f2',
          'text-background-opacity': 0.95,
          'text-background-padding': '3px',
          color: '#991b1b',
          'font-size': '9px',
        },
      },
      {
        selector: 'edge.scenario-value',
        style: {
          'line-color': '#3b82f6',
          'target-arrow-color': '#3b82f6',
          width: 2,
          'z-index': 10,
          label: 'data(label)',
          'text-opacity': 1,
          'text-background-color': '#eff6ff',
          'text-background-opacity': 0.95,
          'text-background-padding': '3px',
          color: '#1e40af',
          'font-size': '9px',
        },
      },
      {
        selector: 'edge.scenario-dimmed',
        style: { opacity: 0.1 },
      },
      {
        selector: 'node.scenario-active',
        style: {
          'border-width': 2.5,
          'border-color': '#154273',
        },
      },
      // User input nodes
      {
        selector: 'node[nodeType="user_input"]',
        style: {
          label: 'data(label)',
          'text-wrap': 'wrap',
          'text-valign': 'center',
          'text-halign': 'center',
          'font-size': '8px',
          'font-family': '"SF Mono", "Fira Code", "Consolas", monospace',
          'background-color': '#f8fafc',
          'background-opacity': 0.9,
          'border-width': 1,
          'border-color': '#94a3b8',
          'border-style': 'dashed',
          shape: 'ellipse',
          width: 'data(nodeWidth)',
          height: 'data(nodeHeight)',
          color: '#475569',
        },
      },
      {
        selector: 'edge[?isUserInput]',
        style: {
          'line-style': 'dotted',
          'line-dash-pattern': [3, 3],
          'line-color': '#cbd5e1',
          'target-arrow-color': '#cbd5e1',
          'target-arrow-shape': 'triangle',
          width: 1,
          'arrow-scale': 0.7,
        },
      },
      {
        selector: 'node.hidden',
        style: { display: 'none' },
      },
      {
        selector: 'edge.hidden-edge',
        style: { display: 'none' },
      },
    ],
    layout: {
      name: 'elk',
      elk: {
        algorithm: 'layered',
        'elk.direction': 'RIGHT',
        'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
        'elk.padding': '[top=40,left=16,bottom=16,right=16]',
        'elk.spacing.nodeNode': '50',
        'elk.layered.spacing.nodeNodeBetweenLayers': '140',
        'elk.layered.spacing.edgeEdgeBetweenLayers': '20',
        'elk.layered.spacing.edgeNodeBetweenLayers': '30',
        'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
        'elk.layered.crossingMinimization.strategy': 'LAYER_SWEEP',
        'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
        'elk.layered.nodePlacement.favorStraightEdges': 'true',
        'elk.partitioning.activate': 'true',
      },
      elkLayoutOptions: (node) => {
        const partition = node.data('elkPartition');
        if (partition !== undefined) {
          return { 'elk.partitioning.partition': String(partition) };
        }
        return {};
      },
      ready: () => {
        cy.fit(undefined, 40);
        restoreAllValidationStatus();
      },
    },
    minZoom: 0.05,
    maxZoom: 4,
    wheelSensitivity: 0.3,
  });

  // Click: highlight article and its connections
  cy.on('tap', 'node[nodeType="article"]', (evt) => {
    const node = evt.target;
    if (selectedNode === node.id()) {
      clearHighlight();
      selectedNode = null;
      hideTooltip();
    } else {
      highlightNode(node.id());
      selectedNode = node.id();
      showTooltip(node, evt.originalEvent);
    }
  });

  // Click law parent: expand/collapse info
  cy.on('tap', 'node[nodeType="law"]', (evt) => {
    if (evt.target.isParent()) {
      const lawId = evt.target.id();
      // Highlight all articles + connections of this law
      if (selectedNode === lawId) {
        clearHighlight();
        selectedNode = null;
        hideTooltip();
      } else {
        highlightLaw(lawId);
        selectedNode = lawId;
      }
    }
  });

  // Double-click article: open in editor
  cy.on('dbltap', 'node[nodeType="article"]', (evt) => {
    const node = evt.target;
    const lawId = node.data('lawId');
    const artNum = node.data('articleNumber');
    window.open(`editor.html?law=${encodeURIComponent(lawId)}&article=${artNum}`, '_blank');
  });

  // Right-click: cycle validation status on law, article, or user_input nodes
  cy.on('cxttap', 'node', (evt) => {
    const node = evt.target;
    const nodeType = node.data('nodeType');
    if (nodeType !== 'law' && nodeType !== 'article' && nodeType !== 'user_input') return;

    const id = node.id();
    const current = nodeStatusMap.get(id) || 'none';
    const nextIdx = (STATUS_CYCLE.indexOf(current) + 1) % STATUS_CYCLE.length;
    const next = STATUS_CYCLE[nextIdx];

    if (next === 'none') {
      nodeStatusMap.delete(id);
    } else {
      nodeStatusMap.set(id, next);
    }
    applyValidationStatus(node, next);
  });

  cy.on('tap', (evt) => {
    if (evt.target === cy) {
      clearHighlight();
      selectedNode = null;
      hideTooltip();
    }
  });

  // Hover tooltip for edges
  cy.on('mouseover', 'edge', (evt) => {
    const edge = evt.target;
    edge.style('text-opacity', 1);
    edge.style('label', edge.data('label'));
  });
  cy.on('mouseout', 'edge', (evt) => {
    const edge = evt.target;
    if (!edge.hasClass('inbound') && !edge.hasClass('outbound') &&
        !edge.hasClass('scenario-true') && !edge.hasClass('scenario-false') &&
        !edge.hasClass('scenario-value')) {
      edge.style('text-opacity', 0);
    }
  });
}

function applyValidationStatus(node, status) {
  const s = VALIDATION_STATUS[status];
  if (status === 'none') {
    // Restore original colors
    const origBg = node.data('bgColor');
    const origBorder = node.data('borderColor');
    if (origBg) node.style('background-color', origBg);
    if (origBorder) node.style('border-color', origBorder);
    if (node.data('nodeType') === 'article') {
      node.style('background-color', '#ffffff');
      node.style('border-color', origBorder);
    }
    if (node.data('nodeType') === 'user_input') {
      node.style('background-color', '#f8fafc');
      node.style('border-color', '#94a3b8');
    }
    node.style('border-width', node.data('nodeType') === 'law' ? 2 : 1.5);
  } else {
    node.style('border-color', s.border);
    node.style('border-width', 3);
    if (node.data('nodeType') === 'law') {
      node.style('background-color', s.bg);
    } else {
      node.style('background-color', s.bg);
    }
  }
}

function restoreAllValidationStatus() {
  if (!cy) return;
  for (const [id, status] of nodeStatusMap) {
    const node = cy.getElementById(id);
    if (node.length) applyValidationStatus(node, status);
  }
}

function highlightNode(nodeId) {
  clearHighlight();
  hideTooltip();

  const node = cy.getElementById(nodeId);
  node.addClass('highlighted');

  const connectedNodeIds = new Set([nodeId]);
  // Also keep parent law visible
  const parentId = node.data('parent');
  if (parentId) connectedNodeIds.add(parentId);

  // Inbound edges
  const inboundEdges = cy.edges(`[target = "${nodeId}"]`);
  inboundEdges.addClass('inbound');
  inboundEdges.forEach(e => {
    connectedNodeIds.add(e.data('source'));
    const srcNode = cy.getElementById(e.data('source'));
    if (srcNode.data('parent')) connectedNodeIds.add(srcNode.data('parent'));
  });

  // Outbound edges
  const outboundEdges = cy.edges(`[source = "${nodeId}"]`);
  outboundEdges.addClass('outbound');
  outboundEdges.forEach(e => {
    connectedNodeIds.add(e.data('target'));
    const tgtNode = cy.getElementById(e.data('target'));
    if (tgtNode.data('parent')) connectedNodeIds.add(tgtNode.data('parent'));
  });

  // Dim unconnected
  cy.nodes().forEach(n => {
    if (!connectedNodeIds.has(n.id())) n.addClass('dimmed');
  });
  cy.edges().forEach(e => {
    if (!e.hasClass('inbound') && !e.hasClass('outbound')) e.addClass('dimmed');
  });
}

function highlightLaw(lawId) {
  clearHighlight();
  hideTooltip();

  const lawNode = cy.getElementById(lawId);
  lawNode.addClass('highlighted');

  const children = lawNode.children();
  const connectedNodeIds = new Set([lawId]);
  children.forEach(c => connectedNodeIds.add(c.id()));

  children.forEach(child => {
    const childId = child.id();
    const inbound = cy.edges(`[target = "${childId}"]`);
    inbound.addClass('inbound');
    inbound.forEach(e => {
      connectedNodeIds.add(e.data('source'));
      const srcNode = cy.getElementById(e.data('source'));
      if (srcNode.data('parent')) connectedNodeIds.add(srcNode.data('parent'));
    });

    const outbound = cy.edges(`[source = "${childId}"]`);
    outbound.addClass('outbound');
    outbound.forEach(e => {
      connectedNodeIds.add(e.data('target'));
      const tgtNode = cy.getElementById(e.data('target'));
      if (tgtNode.data('parent')) connectedNodeIds.add(tgtNode.data('parent'));
    });
  });

  cy.nodes().forEach(n => {
    if (!connectedNodeIds.has(n.id())) n.addClass('dimmed');
  });
  cy.edges().forEach(e => {
    if (!e.hasClass('inbound') && !e.hasClass('outbound')) e.addClass('dimmed');
  });
}

function clearHighlight() {
  if (!cy) return;
  cy.elements().removeClass('highlighted inbound outbound dimmed');
  // Reset edge labels (preserve scenario overlay labels)
  cy.edges().forEach(e => {
    if (!e.hasClass('inbound') && !e.hasClass('outbound') &&
        !e.hasClass('scenario-true') && !e.hasClass('scenario-false') &&
        !e.hasClass('scenario-value')) {
      e.style('text-opacity', 0);
    }
  });
}

function showTooltip(node, mouseEvent) {
  hideTooltip();
  const inputs = node.data('inputs') || [];
  const outputs = node.data('outputs') || [];
  const artNum = node.data('articleNumber');
  const lawId = node.data('lawId');

  let html = `<div class="graph-tooltip">`;
  html += `<div class="graph-tooltip__header">${formatLawName(lawId)}</div>`;
  html += `<div class="graph-tooltip__article">Artikel ${artNum}</div>`;

  if (inputs.length > 0) {
    html += `<div class="graph-tooltip__section">`;
    html += `<div class="graph-tooltip__section-title">Inputs</div>`;
    for (const inp of inputs) {
      const src = inp.source_regulation
        ? `\u2190 ${formatLawName(inp.source_regulation)}`
        : '(parameter)';
      html += `<div class="graph-tooltip__field">`;
      html += `<span class="graph-tooltip__field-name">${inp.name}</span>`;
      html += `<span class="graph-tooltip__field-type">${inp.type}</span>`;
      html += `<span class="graph-tooltip__field-source">${src}</span>`;
      html += `</div>`;
    }
    html += `</div>`;
  }

  if (outputs.length > 0) {
    html += `<div class="graph-tooltip__section">`;
    html += `<div class="graph-tooltip__section-title">Outputs</div>`;
    for (const out of outputs) {
      html += `<div class="graph-tooltip__field">`;
      html += `<span class="graph-tooltip__field-name">${out.name}</span>`;
      html += `<span class="graph-tooltip__field-type">${out.type}</span>`;
      html += `</div>`;
    }
    html += `</div>`;
  }

  html += `<div style="margin-top:8px;font-size:10px;color:#94a3b8;border-top:1px solid #e2e8f0;padding-top:6px;">Dubbelklik \u2192 editor</div>`;
  html += `</div>`;

  const container = document.getElementById('graph-canvas');
  const tooltipEl = document.createElement('div');
  tooltipEl.id = 'graph-tooltip-overlay';
  tooltipEl.innerHTML = html;

  // Position near the mouse click
  const containerRect = container.getBoundingClientRect();
  const x = mouseEvent ? mouseEvent.clientX - containerRect.left + 15 : 20;
  const y = mouseEvent ? mouseEvent.clientY - containerRect.top - 10 : 20;
  tooltipEl.style.position = 'absolute';
  tooltipEl.style.left = `${x}px`;
  tooltipEl.style.top = `${y}px`;
  tooltipEl.style.zIndex = '1000';

  container.appendChild(tooltipEl);
}

function hideTooltip() {
  const existing = document.getElementById('graph-tooltip-overlay');
  if (existing) existing.remove();
}

// ─── Scenario Overlay ───

function parseAllScenarios(features) {
  const scenarios = [];
  for (const feature of features) {
    if (!feature.scenarios) continue;
    for (const s of feature.scenarios) {
      if (s.type !== 'scenario') continue;
      const whenStep = s.steps.find(step => step.keyword === 'When');
      if (!whenStep) continue;

      let match = whenStep.text.match(/the (\S+) is executed for article (.+)/);
      if (!match) {
        match = whenStep.text.match(/the \S+ is executed for (\S+) article (.+)/);
      }
      if (!match) continue;

      const lawId = match[1];
      const article = match[2].trim();

      const inputs = {};
      for (const step of s.steps) {
        if (step.keyword === 'Given' && step.table) {
          for (const [key, value] of step.table) {
            inputs[key] = value;
          }
        }
      }

      const outputs = {};
      for (const step of s.steps) {
        if (step.keyword === 'Then' || step.keyword === 'And') {
          const outMatch = step.text.match(/the output (\S+) is "([^"]+)"/);
          if (outMatch) {
            outputs[outMatch[1]] = outMatch[2];
          }
        }
      }

      scenarios.push({
        name: s.name,
        featureName: feature.name,
        featureFilename: feature.filename,
        lawId,
        article,
        inputs,
        outputs,
        _idx: scenarios.length,
      });
    }
  }
  return scenarios;
}

function buildScenarioFilters() {
  const container = document.getElementById('scenario-filters');
  if (!container || parsedScenarios.length === 0) return;

  container.innerHTML = '';

  const title = document.createElement('div');
  title.className = 'graph-sidebar__section-title';
  title.textContent = "Scenario's";
  container.appendChild(title);

  // Group by feature, then by lawId:article
  const byFeature = {};
  for (const s of parsedScenarios) {
    if (!byFeature[s.featureName]) byFeature[s.featureName] = {};
    const artKey = `${s.lawId}:${s.article}`;
    if (!byFeature[s.featureName][artKey]) byFeature[s.featureName][artKey] = [];
    byFeature[s.featureName][artKey].push(s);
  }

  for (const [featureName, articles] of Object.entries(byFeature)) {
    const featureGroup = document.createElement('div');
    featureGroup.className = 'graph-scenario__feature';

    const featureHeader = document.createElement('button');
    featureHeader.className = 'graph-scenario__feature-header';
    featureHeader.textContent = featureName;
    featureHeader.addEventListener('click', () => {
      featureGroup.classList.toggle('graph-scenario__feature--expanded');
    });
    featureGroup.appendChild(featureHeader);

    const featureBody = document.createElement('div');
    featureBody.className = 'graph-scenario__feature-body';

    for (const [artKey, scenarios] of Object.entries(articles)) {
      const parts = artKey.split(':');
      const lawId = parts[0];
      const artNum = parts.slice(1).join(':');

      const artGroup = document.createElement('div');
      artGroup.className = 'graph-scenario__article-group';

      const artHeader = document.createElement('button');
      artHeader.className = 'graph-scenario__article-header';
      const lawName = formatLawName(lawId);
      const shortName = lawName.length > 25 ? lawName.substring(0, 22) + '...' : lawName;
      artHeader.textContent = `${shortName} art. ${artNum}`;
      artHeader.addEventListener('click', (e) => {
        e.stopPropagation();
        artGroup.classList.toggle('graph-scenario__article-group--expanded');
      });
      artGroup.appendChild(artHeader);

      const artBody = document.createElement('div');
      artBody.className = 'graph-scenario__article-body';

      for (const scenario of scenarios) {
        const btn = document.createElement('button');
        btn.className = 'graph-scenario__item';
        btn.textContent = scenario.name;
        btn.title = scenario.name;
        btn.dataset.scenarioIdx = scenario._idx;
        btn.addEventListener('click', (e) => {
          e.stopPropagation();
          if (activeScenarioIdx === scenario._idx) {
            clearScenarioOverlay();
          } else {
            activateScenarioOverlay(scenario._idx);
          }
        });
        artBody.appendChild(btn);
      }

      artGroup.appendChild(artBody);
      featureBody.appendChild(artGroup);
    }

    featureGroup.appendChild(featureBody);
    container.appendChild(featureGroup);
  }
}

function buildArticleLabelOverlay(art, valueMap) {
  const inputLines = [];
  const outputLines = [];

  for (const inp of art.inputs) {
    const val = valueMap[inp.name];
    inputLines.push(val !== undefined ? `\u2192 ${inp.name} = ${val}` : `\u2192 ${inp.name}`);
  }
  for (const out of art.outputs) {
    const val = valueMap[out.name];
    outputLines.push(val !== undefined ? `\u2190 ${out.name} = ${val}` : `\u2190 ${out.name}`);
  }

  const allLines = [...inputLines, ...outputLines, `Art. ${art.number}`];
  const maxLen = Math.max(...allLines.map(l => l.length));
  const sep = '\u2500'.repeat(Math.max(16, maxLen));

  const lines = [`Art. ${art.number}`];
  if (inputLines.length > 0 || outputLines.length > 0) {
    lines.push(sep);
  }
  lines.push(...inputLines);
  if (inputLines.length > 0 && outputLines.length > 0) {
    lines.push(sep);
  }
  lines.push(...outputLines);
  return lines.join('\n');
}

function calcLabelSize(label) {
  const lines = label.split('\n');
  const height = Math.max(40, lines.length * 16 + 16);
  const maxLen = Math.max(...lines.map(l => l.length));
  const width = Math.max(120, maxLen * 7.5 + 24);
  return { width, height };
}

function activateScenarioOverlay(idx) {
  clearScenarioOverlay();
  if (!cy || idx == null) return;

  activeScenarioIdx = idx;
  const scenario = parsedScenarios[idx];
  if (!scenario) return;

  // Update active button styling
  document.querySelectorAll('.graph-scenario__item').forEach(btn => {
    btn.classList.toggle(
      'graph-scenario__item--active',
      parseInt(btn.dataset.scenarioIdx) === idx
    );
  });

  // Build per-article value maps from feature scenarios
  const featureScenarios = parsedScenarios.filter(
    s => s.featureFilename === scenario.featureFilename
  );

  const articleValueMaps = {};
  for (const s of featureScenarios) {
    const key = `${s.lawId}:${s.article}`;
    if (s._idx === idx) {
      // Selected scenario takes priority for its article
      articleValueMaps[key] = { ...s.inputs, ...s.outputs };
    } else if (!articleValueMaps[key]) {
      articleValueMaps[key] = { ...s.inputs, ...s.outputs };
    }
  }

  // Global value map for edge lookups
  const globalValueMap = {};
  for (const s of featureScenarios) {
    Object.assign(globalValueMap, s.inputs);
    Object.assign(globalValueMap, s.outputs);
  }
  Object.assign(globalValueMap, scenario.inputs);
  Object.assign(globalValueMap, scenario.outputs);

  // Propagate values between input names and their source_output names,
  // so a value set under one name also applies to the other.
  cy.nodes('[nodeType="article"]').forEach(node => {
    const inputs = node.data('inputs') || [];
    for (const inp of inputs) {
      if (inp.source_output) {
        if (globalValueMap[inp.name] !== undefined && globalValueMap[inp.source_output] === undefined) {
          globalValueMap[inp.source_output] = globalValueMap[inp.name];
        }
        if (globalValueMap[inp.source_output] !== undefined && globalValueMap[inp.name] === undefined) {
          globalValueMap[inp.name] = globalValueMap[inp.source_output];
        }
      }
    }
  });

  // Update article node labels
  cy.nodes('[nodeType="article"]').forEach(node => {
    const lawId = node.data('lawId');
    const artNum = node.data('articleNumber');
    const key = `${lawId}:${artNum}`;
    const valueMap = articleValueMaps[key] || globalValueMap;

    const inputs = node.data('inputs') || [];
    const outputs = node.data('outputs') || [];

    const hasValues = inputs.some(i => valueMap[i.name] !== undefined) ||
                      outputs.some(o => valueMap[o.name] !== undefined);

    if (hasValues) {
      node.data('_origLabel', node.data('label'));
      node.data('_origWidth', node.data('nodeWidth'));
      node.data('_origHeight', node.data('nodeHeight'));

      const newLabel = buildArticleLabelOverlay(
        { number: artNum, inputs, outputs }, valueMap
      );
      const newSize = calcLabelSize(newLabel);

      node.data('label', newLabel);
      node.data('nodeWidth', newSize.width);
      node.data('nodeHeight', newSize.height);
      node.addClass('scenario-active');
    }
  });

  // Update user input node labels
  cy.nodes('[nodeType="user_input"]').forEach(node => {
    const fieldName = node.data('fieldName');
    const value = globalValueMap[fieldName];
    if (value !== undefined) {
      node.data('_origLabel', node.data('label'));
      node.data('_origWidth', node.data('nodeWidth'));
      const valLabel = `${node.data('label')}\n= ${value}`;
      const valWidth = Math.max(node.data('nodeWidth'), valLabel.length * 4 + 20);
      node.data('label', valLabel);
      node.data('nodeWidth', valWidth);
      node.data('nodeHeight', 38);
      node.addClass('scenario-active');
    }
  });

  // Update edge labels and colors
  cy.edges().forEach(edge => {
    if (edge.data('isLegalBasis')) {
      edge.addClass('scenario-dimmed');
      return;
    }

    const fieldName = edge.data('sourceOutput');
    if (!fieldName) {
      edge.addClass('scenario-dimmed');
      return;
    }

    const value = globalValueMap[fieldName];

    if (value !== undefined) {
      edge.data('_origLabel', edge.data('label'));
      edge.data('label', `${fieldName} = ${value}`);

      if (value === 'true') {
        edge.addClass('scenario-true');
      } else if (value === 'false') {
        edge.addClass('scenario-false');
      } else {
        edge.addClass('scenario-value');
      }
    } else {
      edge.addClass('scenario-dimmed');
    }
  });
}

function clearScenarioOverlay() {
  activeScenarioIdx = null;

  document.querySelectorAll('.graph-scenario__item--active').forEach(btn => {
    btn.classList.remove('graph-scenario__item--active');
  });

  if (!cy) return;

  cy.nodes('[nodeType="article"], [nodeType="user_input"]').forEach(node => {
    if (node.data('_origLabel') !== undefined) {
      node.data('label', node.data('_origLabel'));
      node.data('nodeWidth', node.data('_origWidth'));
      node.removeData('_origLabel');
      node.removeData('_origWidth');
      if (node.data('_origHeight') !== undefined) {
        node.data('nodeHeight', node.data('_origHeight'));
        node.removeData('_origHeight');
      }
    }
    node.removeClass('scenario-active');
  });

  cy.edges().forEach(edge => {
    if (edge.data('_origLabel') !== undefined) {
      edge.data('label', edge.data('_origLabel'));
      edge.removeData('_origLabel');
    }
    edge.removeClass('scenario-true scenario-false scenario-value scenario-dimmed');
  });
}

function updateVisibility() {
  const checkboxes = document.querySelectorAll('#law-filters input[type="checkbox"]');
  const visibleLawIds = new Set();
  checkboxes.forEach(cb => {
    if (cb.checked) visibleLawIds.add(cb.value);
  });

  cy.nodes().forEach(n => {
    const lawId = n.data('nodeType') === 'law' ? n.id() : n.data('lawId');
    if (visibleLawIds.has(lawId)) {
      n.removeClass('hidden');
    } else {
      n.addClass('hidden');
    }
  });

  cy.edges().forEach(e => {
    const sourceLaw = e.data('sourceLaw') || e.data('source');
    const targetLaw = e.data('targetLaw') || e.data('target');
    if (visibleLawIds.has(sourceLaw) && visibleLawIds.has(targetLaw)) {
      e.removeClass('hidden-edge');
    } else {
      e.addClass('hidden-edge');
    }
  });
}

// ─── YAML Upload ───

function parseYamlToGraphData(docs) {
  const laws = [];
  const edges = [];

  for (const doc of docs) {
    if (!doc || !doc.$id) continue;

    const rawName = doc.name && !doc.name.startsWith('#') ? doc.name : doc.$id;
    const law = {
      id: doc.$id,
      name: rawName,
      regulatory_layer: doc.regulatory_layer || 'ONBEKEND',
      legal_basis: doc.legal_basis || null,
      bwb_id: doc.bwb_id || null,
      url: doc.url || null,
      valid_from: doc.valid_from || null,
      articles: [],
    };

    if (doc.articles) {
      for (const article of doc.articles) {
        const artInfo = { number: article.number, outputs: [], inputs: [] };
        const mr = article.machine_readable;
        if (mr && mr.execution) {
          if (mr.execution.output) {
            for (const out of mr.execution.output) {
              artInfo.outputs.push({ name: out.name, type: out.type });
            }
          }
          if (mr.execution.input) {
            for (const inp of mr.execution.input) {
              artInfo.inputs.push({
                name: inp.name,
                type: inp.type,
                source_regulation: inp.source?.regulation || null,
                source_output: inp.source?.output || null,
              });

              if (inp.source?.regulation && inp.source.regulation !== doc.$id) {
                edges.push({
                  source: inp.source.regulation,
                  target: doc.$id,
                  source_output: inp.source.output || inp.name,
                  target_input: inp.name,
                  target_article: article.number,
                  source_article: null,
                });
              }
            }
          }
        }
        if (artInfo.outputs.length > 0 || artInfo.inputs.length > 0) {
          law.articles.push(artInfo);
        }
      }
    }

    laws.push(law);
  }

  // Build output lookup
  const outputLookup = {};
  for (const law of laws) {
    outputLookup[law.id] = {};
    for (const art of law.articles) {
      for (const out of art.outputs) {
        outputLookup[law.id][out.name] = art.number;
      }
    }
  }

  // Resolve source_article
  for (const edge of edges) {
    if (edge.source_article === null && outputLookup[edge.source]) {
      edge.source_article = outputLookup[edge.source][edge.source_output] || null;
    }
  }

  // Add intra-law edges
  for (const law of laws) {
    for (const art of law.articles) {
      for (const inp of art.inputs) {
        if (inp.source_output && !inp.source_regulation) {
          const sourceArt = outputLookup[law.id]?.[inp.source_output];
          if (sourceArt && sourceArt !== art.number) {
            edges.push({
              source: law.id,
              target: law.id,
              source_article: sourceArt,
              target_article: art.number,
              source_output: inp.source_output,
              target_input: inp.name,
            });
          }
        }
      }
    }
  }

  // Add legal_basis edges
  for (const law of laws) {
    if (law.legal_basis) {
      const seen = new Set();
      for (const lb of law.legal_basis) {
        if (!lb.law_id || seen.has(lb.law_id)) continue;
        seen.add(lb.law_id);
        edges.push({
          source: lb.law_id,
          target: law.id,
          source_article: lb.article || null,
          target_article: null,
          source_output: 'legal_basis',
          target_input: 'legal_basis',
          edge_type: 'legal_basis',
        });
      }
    }
  }

  // Deduplicate
  const edgeSet = new Set();
  const uniqueEdges = edges.filter(e => {
    const key = `${e.source}:${e.source_article}->${e.target}:${e.target_article}:${e.source_output}`;
    if (edgeSet.has(key)) return false;
    edgeSet.add(key);
    return true;
  });

  return { laws, edges: uniqueEdges };
}

function mergeAndRender(uploadedData, fileCount) {
  if (graphData) {
    const existingIds = new Set(graphData.laws.map(l => l.id));
    for (const law of uploadedData.laws) {
      if (existingIds.has(law.id)) {
        const idx = graphData.laws.findIndex(l => l.id === law.id);
        graphData.laws[idx] = law;
      } else {
        graphData.laws.push(law);
      }
    }
    graphData.edges = graphData.edges.concat(uploadedData.edges);
    const edgeSet = new Set();
    graphData.edges = graphData.edges.filter(e => {
      const key = `${e.source}:${e.source_article}->${e.target}:${e.target_article}:${e.source_output}`;
      if (edgeSet.has(key)) return false;
      edgeSet.add(key);
      return true;
    });
  } else {
    graphData = uploadedData;
  }

  buildScenarioFilters();
  buildFilters();
  renderGraph(graphData);

  const status = document.getElementById('upload-status');
  if (status) {
    status.textContent = `${fileCount} bestand${fileCount !== 1 ? 'en' : ''} geladen`;
    setTimeout(() => { status.textContent = ''; }, 3000);
  }
}

async function loadYamlFiles(files) {
  const docs = [];
  for (const file of files) {
    if (!file.name.endsWith('.yaml') && !file.name.endsWith('.yml')) continue;
    try {
      const text = await file.text();
      const doc = jsyaml.load(text);
      if (doc) docs.push(doc);
    } catch (e) {
      console.warn(`Could not parse ${file.name}: ${e.message}`);
    }
  }
  if (docs.length === 0) return;
  mergeAndRender(parseYamlToGraphData(docs), docs.length);
}

function setupYamlUpload() {
  const input = document.getElementById('yaml-upload');
  if (input) {
    input.addEventListener('change', (evt) => loadYamlFiles(evt.target.files));
  }

  const folderInput = document.getElementById('yaml-folder-upload');
  if (folderInput) {
    folderInput.addEventListener('change', (evt) => loadYamlFiles(evt.target.files));
  }

  // Drag & drop on the canvas
  const canvas = document.getElementById('graph-canvas');
  if (canvas) {
    canvas.addEventListener('dragover', (e) => {
      e.preventDefault();
      canvas.classList.add('graph-canvas--dragover');
    });
    canvas.addEventListener('dragleave', () => {
      canvas.classList.remove('graph-canvas--dragover');
    });
    canvas.addEventListener('drop', (e) => {
      e.preventDefault();
      canvas.classList.remove('graph-canvas--dragover');
      const files = e.dataTransfer.files;
      if (files.length > 0) loadYamlFiles(files);
    });
  }
}

// Disable browser context menu on graph canvas so right-click cycles status
document.getElementById('graph-canvas')?.addEventListener('contextmenu', (e) => e.preventDefault());

// Init
init();
setupYamlUpload();
