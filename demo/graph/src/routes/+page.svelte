<script lang="ts">
  import yaml from 'yaml';
  import { untrack } from 'svelte';
  import {
    MarkerType,
    SvelteFlow,
    Controls,
    Background,
    BackgroundVariant,
    MiniMap,
    type Node,
    type Edge,
    Position,
  } from '@xyflow/svelte';
  import LawNode from './LawNode.svelte';
  import LeafNode from './LeafNode.svelte';
  import { initEngine, type WasmEngine } from '$lib/wasmEngine';
  import {
    parseFeature,
    runScenario,
    type DemoScenario,
    type ScenarioRunResult,
  } from '$lib/demoGherkin';
  import {
    flattenTraceSteps,
    edgeIdsForStep,
    graphNodeIdsForStep,
    type Step,
  } from '$lib/traceEdges';

  // Import the styles for Svelte Flow to work
  import '@xyflow/svelte/dist/style.css';

  /**
   * Regelrecht YAML schema mapping.
   *
   * Each YAML file has:
   *   $id, name, regulatory_layer, valid_from, articles[]
   *
   * Each article may have machine_readable.execution with:
   *   parameters[], input[] (with source.regulation/output), output[]
   *
   * We flatten articles with machine_readable into a single node per law,
   * showing parameters as "sources", input as "input" (with cross-law refs),
   * and output as "output".
   */

  type LawInput = {
    name: string;
    source_regulation?: string;
    source_output?: string;
  };

  type LawArticle = {
    number: string;
    parameters: string[];
    input: LawInput[];
    output: string[];
    implements: { law: string; open_term: string }[];
    overrides: { law: string; article: string; output: string }[];
    hooks: string[];
    open_terms: string[];
    produces_legal_character?: string;
    text?: string;
  };

  type Law = {
    id: string;
    name: string;
    layer: string;
    valid_from: string;
    articles: LawArticle[];
  };

  let nodes = $state.raw<Node[]>([]);
  let edges = $state.raw<Edge[]>([]);

  const nodeTypes: any = {
    law: LawNode,
    leaf: LeafNode,
  };

  // Map regulatory layers to color indices
  const layerToColorIndex: Record<string, number> = {
    WET: 0,
    AMVB: 1,
    MINISTERIELE_REGELING: 2,
    GEMEENTELIJKE_VERORDENING: 3,
    GRONDWET: 4,
    BELEIDSREGEL: 5,
    EU_VERORDENING: 6,
  };

  function getLayerColorIndex(layer: string): number {
    return layerToColorIndex[layer] ?? 0;
  }

  let laws = $state<Law[]>([]);
  let selectedLaws = $state<string[]>([]);
  let selectedRootNode = $state<string | null>(null);

  // --- Scenario runner state ---
  let rawYamlById = new Map<string, string>(); // filled during initial load
  let features = $state<string[]>([]);
  let selectedFeature = $state<string | null>(null);
  let featureScenarios = $state<DemoScenario[]>([]);
  let selectedScenarioIdx = $state<number>(0);
  let running = $state<boolean>(false);
  let runError = $state<string | null>(null);
  let traceRun = $state<ScenarioRunResult | null>(null);
  let traceSteps = $state<Step[]>([]);
  let currentStepIdx = $state<number>(-1);
  let traceView = $state<'steps' | 'tree'>('steps');
  let traceFilter = $state<'highlights' | 'all'>('highlights');
  let traceStartNodeIds = $state<string[]>([]); // root law + initial output leaf

  function parseLaw(yamlContent: string): Law | null {
    const data = yaml.parse(yamlContent);
    if (!data || !data.$id) return null;

    const articles: LawArticle[] = [];

    for (const art of data.articles || []) {
      const mr = art.machine_readable;
      if (!mr) continue;
      const ex = mr.execution || {};

      const inputs: LawInput[] = [];
      for (const inp of ex.input || []) {
        const li: LawInput = { name: inp.name };
        const src = inp.source;
        if (src && typeof src === 'object') {
          if (src.regulation) {
            li.source_regulation = src.regulation;
            li.source_output = src.output || inp.name;
          } else if (src.output) {
            li.source_output = src.output;
          }
        }
        inputs.push(li);
      }

      articles.push({
        number: String(art.number),
        parameters: (ex.parameters || []).map((p: any) => p.name),
        input: inputs,
        output: (ex.output || []).map((o: any) => o.name),
        implements: (mr.implements || []).map((i: any) => ({ law: i.law, open_term: i.open_term })),
        overrides: (mr.overrides || []).map((o: any) => ({ law: o.law, article: o.article, output: o.output })),
        hooks: (mr.hooks || []).map((h: any) => h.applies_to?.legal_character || ''),
        open_terms: (mr.open_terms || []).map((ot: any) => ot.id),
        produces_legal_character: ex.produces?.legal_character,
        text: art.text || '',
      });
    }

    return {
      id: data.$id,
      name: typeof data.name === 'string' && data.name.startsWith('#') ? data.$id.replace(/_/g, ' ') : data.name,
      layer: data.regulatory_layer || 'WET',
      valid_from: data.valid_from || '',
      articles,
    };
  }

  function getTransitiveDeps(rootId: string, allLaws: Law[]): Set<string> {
    const deps = new Set<string>([rootId]);
    const queue = [rootId];

    // Phase 1: walk source references, implements, overrides (no hooks yet)
    while (queue.length > 0) {
      const current = queue.shift()!;
      const law = allLaws.find(l => l.id === current);
      if (!law) continue;

      for (const art of law.articles) {
        for (const inp of art.input) {
          if (inp.source_regulation && !deps.has(inp.source_regulation)) {
            deps.add(inp.source_regulation);
            queue.push(inp.source_regulation);
          }
        }
        for (const impl of art.implements) {
          if (!deps.has(impl.law)) { deps.add(impl.law); queue.push(impl.law); }
        }
        for (const ovr of art.overrides) {
          if (!deps.has(ovr.law)) { deps.add(ovr.law); queue.push(ovr.law); }
        }
      }

      // Reverse: laws that depend on current
      for (const other of allLaws) {
        if (deps.has(other.id)) continue;
        let found = false;
        for (const art of other.articles) {
          for (const inp of art.input) {
            if (inp.source_regulation === current) { found = true; break; }
          }
          if (!found) for (const impl of art.implements) {
            if (impl.law === current) { found = true; break; }
          }
          if (!found) for (const ovr of art.overrides) {
            if (ovr.law === current) { found = true; break; }
          }
          if (found) break;
        }
        if (found) { deps.add(other.id); queue.push(other.id); }
      }
    }

    // Phase 2: for laws already in deps that produce a legal_character,
    // add ONLY the hook laws (not their transitive deps — those are cross-cutting)
    const producedCharacters = new Set<string>();
    for (const lawId of deps) {
      const law = allLaws.find(l => l.id === lawId);
      if (!law) continue;
      for (const art of law.articles) {
        if (art.produces_legal_character) producedCharacters.add(art.produces_legal_character);
      }
    }

    if (producedCharacters.size > 0) {
      const hookLaws: string[] = [];
      for (const other of allLaws) {
        if (deps.has(other.id)) continue;
        for (const art of other.articles) {
          for (const hook of art.hooks) {
            if (hook && producedCharacters.has(hook)) {
              hookLaws.push(other.id);
              break;
            }
          }
        }
      }
      // Add hook laws and their direct source dependencies (but don't recurse into hooks again)
      for (const hlId of hookLaws) {
        deps.add(hlId);
        const hl = allLaws.find(l => l.id === hlId);
        if (!hl) continue;
        for (const art of hl.articles) {
          for (const inp of art.input) {
            if (inp.source_regulation && !deps.has(inp.source_regulation)) {
              deps.add(inp.source_regulation);
            }
          }
        }
      }
    }

    return deps;
  }

  (async () => {
    try {
      const response = await fetch('/laws/list');
      const filePaths: string[] = await response.json();

      const allLaws: Law[] = [];
      const yamlByLawKey = new Map<string, { yaml: string; valid_from: string }>();
      await Promise.all(
        filePaths.map(async (filePath) => {
          const fileContent = await fetch(`/law/${filePath}`).then((r) => r.text());
          const law = parseLaw(fileContent);
          if (law) {
            allLaws.push(law);
            const existing = yamlByLawKey.get(law.id);
            if (!existing || law.valid_from > existing.valid_from) {
              yamlByLawKey.set(law.id, { yaml: fileContent, valid_from: law.valid_from });
            }
          }
        }),
      );

      // Deduplicate: keep latest version per $id
      const lawMap = new Map<string, Law>();
      for (const law of allLaws) {
        const existing = lawMap.get(law.id);
        if (!existing || law.valid_from > existing.valid_from) {
          lawMap.set(law.id, law);
        }
      }

      laws = Array.from(lawMap.values());
      rawYamlById = new Map(
        Array.from(yamlByLawKey.entries()).map(([id, v]) => [id, v.yaml]),
      );

      // Build output index: "law_id:output_name" → law_id[]
      const serviceOutputToIDs = new Map<string, string[]>();
      for (const law of laws) {
        for (const art of law.articles) {
          for (const out of art.output) {
            const key = `${law.id}:${out}`;
            const cur = serviceOutputToIDs.get(key) || [];
            cur.push(law.id);
            serviceOutputToIDs.set(key, cur);
          }
        }
      }

      // Focus mode: ?focus=law_id shows only that law and its transitive deps
      const focusParam = typeof window !== 'undefined'
        ? new URLSearchParams(window.location.search).get('focus')
        : null;

      if (focusParam) {
        const focusIds = getTransitiveDeps(focusParam, laws);
        selectedLaws = [...focusIds];
      } else {
        selectedLaws = laws.filter(l => l.articles.length > 0).map((law) => law.id);
      }

      const ns: Node[] = [];
      const es: Edge[] = [];
      let i = 0;

      // First pass: create all nodes
      for (const law of laws) {
        // Skip laws with no executable articles
        if (law.articles.length === 0) continue;

        const colorIndex = getLayerColorIndex(law.layer);

        // Collect all unique parameters, inputs, outputs, delegates, implements across articles
        const allParams = new Set<string>();
        const allInputs: { name: string; ref?: { regulation: string; output: string } }[] = [];
        const allOutputs = new Set<string>();
        const allDelegates = new Set<string>(); // open_terms this law declares (delegates to lower regulation)
        const allImplements = new Set<string>(); // open_terms this law implements (from higher law)
        const inputsSeen = new Set<string>();
        // Maps field name → { article number, article text }
        const fieldProvenance = new Map<string, { artNumber: string; text: string }>();

        for (const art of law.articles) {
          for (const p of art.parameters) {
            allParams.add(p);
            fieldProvenance.set(p, { artNumber: art.number, text: art.text || '' });
          }
          for (const out of art.output) {
            allOutputs.add(out);
            fieldProvenance.set(out, { artNumber: art.number, text: art.text || '' });
          }
          for (const ot of art.open_terms) {
            allDelegates.add(ot);
            fieldProvenance.set(ot, { artNumber: art.number, text: art.text || '' });
          }
          for (const impl of art.implements) {
            allImplements.add(impl.open_term);
            fieldProvenance.set(`impl:${impl.open_term}`, { artNumber: art.number, text: art.text || '' });
          }
          // Also collect open_terms that OTHER laws implement for THIS law
          for (const otherLaw of laws) {
            for (const otherArt of otherLaw.articles) {
              for (const impl of otherArt.implements) {
                if (impl.law === law.id) allDelegates.add(impl.open_term);
              }
            }
          }
          for (const inp of art.input) {
            if (inputsSeen.has(inp.name)) continue;
            inputsSeen.add(inp.name);
            fieldProvenance.set(`input:${inp.name}`, { artNumber: art.number, text: art.text || '' });
            if (inp.source_regulation && inp.source_regulation !== law.id) {
              allInputs.push({
                name: inp.name,
                ref: { regulation: inp.source_regulation, output: inp.source_output || inp.name },
              });
            } else {
              allInputs.push({ name: inp.name });
            }
          }
        }

        // Filter out utility outputs and anything that's a delegate (shown separately)
        const filteredOutputs = [...allOutputs].filter(
          (o) => !['wet_naam', 'bevoegd_gezag', 'datum_inwerkingtreding'].includes(o) && !allDelegates.has(o),
        );
        const filteredParams = [...allParams];
        const filteredDelegates = [...allDelegates];
        const filteredImplements = [...allImplements];

        // Calculate height: left column (params + inputs + implements) vs right column (outputs + delegates)
        const leftCount = filteredParams.length + allInputs.length + filteredImplements.length;
        const rightCount = filteredOutputs.length + filteredDelegates.length;
        const leftHeight = leftCount * 50 + (filteredParams.length > 0 ? 70 : 0) + (allInputs.length > 0 ? 70 : 0) + (filteredImplements.length > 0 ? 70 : 0);
        const rightHeight = rightCount * 50 + (filteredOutputs.length > 0 ? 70 : 0) + (filteredDelegates.length > 0 ? 70 : 0);

        ns.push({
          id: law.id,
          type: 'law',
          data: { label: `${law.layer} — ${law.name}` },
          position: { x: i++ * 400, y: 0 },
          width: 500,
          height: Math.max(leftHeight, rightHeight) + 120,
          class: `root service-${colorIndex}`,
          selectable: false,
        });

        // Sources (parameters)
        const sourcesID = `${law.id}-sources`;
        if (filteredParams.length > 0) {
          ns.push({
            id: sourcesID,
            type: 'default',
            data: { label: 'Parameters' },
            position: { x: 10, y: 60 },
            width: 220,
            height: filteredParams.length * 50 + 50,
            parentId: law.id,
            class: `property-group service-${colorIndex}`,
            draggable: false,
            selectable: false,
          });

          let j = 0;
          for (const param of filteredParams) {
            const prov = fieldProvenance.get(param);
            ns.push({
              id: `${law.id}-source-${param}`,
              type: 'leaf',
              sourcePosition: Position.Left,
              data: { label: param, tooltip: prov ? `Art. ${prov.artNumber}\n\n${prov.text}` : '' },
              position: { x: 10, y: (j++ + 1) * 50 },
              width: 200,
              height: 40,
              parentId: sourcesID,
              draggable: false,
              selectable: false,
            });
          }
        }

        // Input
        const inputsID = `${law.id}-input`;
        const inputYOffset = filteredParams.length > 0 ? filteredParams.length * 50 + 130 : 60;

        if (allInputs.length > 0) {
          ns.push({
            id: inputsID,
            type: 'default',
            data: { label: 'Input' },
            position: { x: 10, y: inputYOffset },
            width: 220,
            height: allInputs.length * 50 + 50,
            parentId: law.id,
            class: `property-group service-${colorIndex}`,
            draggable: false,
            selectable: false,
          });

          let j = 0;
          for (const input of allInputs) {
            const iProv = fieldProvenance.get(`input:${input.name}`);
            ns.push({
              id: `${law.id}-input-${input.name}`,
              type: 'leaf',
              sourcePosition: Position.Left,
              data: { label: input.name, tooltip: iProv ? `Art. ${iProv.artNumber}\n\n${iProv.text}` : '' },
              position: { x: 10, y: (j++ + 1) * 50 },
              width: 200,
              height: 40,
              parentId: inputsID,
              extent: 'parent',
              draggable: false,
              selectable: false,
            });
          }
        }

        // Output
        const outputsID = `${law.id}-output`;
        if (filteredOutputs.length > 0) {
          ns.push({
            id: outputsID,
            type: 'default',
            data: { label: 'Output' },
            position: { x: 250, y: 60 },
            width: 220,
            height: filteredOutputs.length * 50 + 50,
            parentId: law.id,
            class: `property-group service-${colorIndex}`,
            draggable: false,
            selectable: false,
          });

          let j = 0;
          for (const output of filteredOutputs) {
            const oProv = fieldProvenance.get(output);
            ns.push({
              id: `${law.id}-output-${output}`,
              type: 'leaf',
              sourcePosition: Position.Right,
              targetPosition: Position.Right,
              data: { label: output, tooltip: oProv ? `Art. ${oProv.artNumber}\n\n${oProv.text}` : '' },
              position: { x: 10, y: (j++ + 1) * 50 },
              width: 200,
              height: 40,
              parentId: outputsID,
              extent: 'parent',
              draggable: false,
              selectable: false,
            });
          }
        }

        // Delegates (open_terms — delegation to lower regulation)
        const delegatesYOffset = filteredOutputs.length > 0
          ? 60 + filteredOutputs.length * 50 + 70
          : 60;

        if (filteredDelegates.length > 0) {
          const delegatesID = `${law.id}-delegates`;
          ns.push({
            id: delegatesID,
            type: 'default',
            data: { label: 'Delegeert' },
            position: { x: 250, y: delegatesYOffset },
            width: 220,
            height: filteredDelegates.length * 50 + 50,
            parentId: law.id,
            class: `property-group service-${colorIndex}`,
            draggable: false,
            selectable: false,
          });

          let j = 0;
          for (const del of filteredDelegates) {
            const dProv = fieldProvenance.get(del);
            ns.push({
              id: `${law.id}-delegate-${del}`,
              type: 'leaf',
              sourcePosition: Position.Right,
              targetPosition: Position.Right,
              data: { label: del, tooltip: dProv ? `Art. ${dProv.artNumber}\n\n${dProv.text}` : '' },
              position: { x: 10, y: (j++ + 1) * 50 },
              width: 200,
              height: 40,
              parentId: delegatesID,
              extent: 'parent',
              draggable: false,
              selectable: false,
            });
          }
        }

        // Implements (this law implements open_terms from a higher law)
        const implementsYOffset = filteredParams.length > 0 || allInputs.length > 0
          ? (filteredParams.length > 0 ? filteredParams.length * 50 + 130 : 60) +
            (allInputs.length > 0 ? allInputs.length * 50 + 70 : 0)
          : 60;

        if (filteredImplements.length > 0) {
          const implementsID = `${law.id}-implements`;
          ns.push({
            id: implementsID,
            type: 'default',
            data: { label: 'Implementeert' },
            position: { x: 10, y: implementsYOffset },
            width: 220,
            height: filteredImplements.length * 50 + 50,
            parentId: law.id,
            class: `property-group service-${colorIndex}`,
            draggable: false,
            selectable: false,
          });

          let j = 0;
          for (const impl of filteredImplements) {
            const imProv = fieldProvenance.get(`impl:${impl}`);
            ns.push({
              id: `${law.id}-impl-${impl}`,
              type: 'leaf',
              sourcePosition: Position.Left,
              data: { label: impl, tooltip: imProv ? `Art. ${imProv.artNumber}\n\n${imProv.text}` : '' },
              position: { x: 10, y: (j++ + 1) * 50 },
              width: 200,
              height: 40,
              parentId: implementsID,
              extent: 'parent',
              draggable: false,
              selectable: false,
            });
          }
        }
      }

      // Second pass: create edges
      for (const law of laws) {
        for (const art of law.articles) {
          // Cross-law source references
          for (const input of art.input) {
            if (!input.source_regulation || input.source_regulation === law.id) continue;

            const inputID = `${law.id}-input-${input.name}`;
            const key = `${input.source_regulation}:${input.source_output || input.name}`;

            for (const targetLawId of serviceOutputToIDs.get(key) || []) {
              const target = `${targetLawId}-output-${input.source_output || input.name}`;
              es.push({
                id: `${inputID}->${target}`,
                source: inputID,
                target: target,
                data: { refersToService: input.source_regulation },
                type: 'bezier',
                markerEnd: { type: MarkerType.ArrowClosed, width: 20, height: 40 },
                zIndex: 2,
              });
            }
          }

          // Implements edges — connect impl node to delegate node
          for (const impl of art.implements) {
            const sourceId = `${law.id}-impl-${impl.open_term}`;
            const targetId = `${impl.law}-delegate-${impl.open_term}`;

            if (ns.find(n => n.id === sourceId) && ns.find(n => n.id === targetId)) {
              const edgeId = `impl:${law.id}:${art.number}->${impl.law}:${impl.open_term}`;
              if (!es.find(e => e.id === edgeId)) {
                es.push({
                  id: edgeId,
                  source: sourceId,
                  target: targetId,
                  type: 'bezier',
                  markerEnd: { type: MarkerType.ArrowClosed, width: 20, height: 40 },
                  style: 'stroke: #10b981; stroke-width: 3; stroke-dasharray: 8 4;',
                  zIndex: 2,
                  label: `implements`,
                  labelStyle: 'fill: #065f46; font-weight: 600;',
                });
              }
            }
          }

          // Override edges
          for (const ovr of art.overrides) {
            const targetLaw = laws.find((l) => l.id === ovr.law);
            if (!targetLaw) continue;

            const sourceId = `${law.id}-output-${ovr.output}`;
            const targetId = `${ovr.law}-output-${ovr.output}`;

            if (ns.find(n => n.id === sourceId) && ns.find(n => n.id === targetId)) {
              const edgeId = `ovr:${law.id}:${art.number}->${ovr.law}:${ovr.article}`;
              if (!es.find(e => e.id === edgeId)) {
                es.push({
                  id: edgeId,
                  source: sourceId,
                  target: targetId,
                  type: 'bezier',
                  markerEnd: { type: MarkerType.ArrowClosed, width: 20, height: 40 },
                  style: 'stroke: #ef4444; stroke-width: 3; stroke-dasharray: 4 4;',
                  zIndex: 2,
                  label: `overrides`,
                  labelStyle: 'fill: #991b1b; font-weight: 600;',
                });
              }
            }
          }
        }
      }

      // Hook edges: AWB articles with hooks fire on laws that produce matching legal_character
      // Collect all laws/articles that produce a legal_character
      const producers: { lawId: string; artNumber: string; legalCharacter: string }[] = [];
      for (const law of laws) {
        for (const art of law.articles) {
          if (art.produces_legal_character) {
            producers.push({ lawId: law.id, artNumber: art.number, legalCharacter: art.produces_legal_character });
          }
        }
      }

      // Find hook articles and connect them to producers
      // Hook source: first output node of the hook law
      // Hook target: first output node of the producer law
      for (const law of laws) {
        for (const art of law.articles) {
          for (const hookTarget of art.hooks) {
            if (!hookTarget) continue;
            // Find a source node (first output of the hook article)
            const hookOutputs = art.output.filter(o => !['wet_naam', 'bevoegd_gezag'].includes(o));
            const sourceId = hookOutputs.length > 0
              ? `${law.id}-output-${hookOutputs[0]}`
              : null;
            if (!sourceId || !ns.find(n => n.id === sourceId)) continue;

            for (const producer of producers) {
              if (producer.legalCharacter === hookTarget && producer.lawId !== law.id) {
                // Find a target node (first output of the producer)
                const producerLaw = laws.find(l => l.id === producer.lawId);
                if (!producerLaw) continue;
                const producerArt = producerLaw.articles.find(a => a.number === producer.artNumber);
                if (!producerArt) continue;
                const targetOutputs = producerArt.output.filter(o => !['wet_naam', 'bevoegd_gezag'].includes(o));
                const targetId = targetOutputs.length > 0
                  ? `${producer.lawId}-output-${targetOutputs[0]}`
                  : null;
                if (!targetId || !ns.find(n => n.id === targetId)) continue;

                const edgeId = `hook:${law.id}:${art.number}->${producer.lawId}:${producer.artNumber}`;
                if (!es.find(e => e.id === edgeId)) {
                  es.push({
                    id: edgeId,
                    source: sourceId,
                    target: targetId,
                    type: 'bezier',
                    markerEnd: { type: MarkerType.ArrowClosed, width: 20, height: 40 },
                    style: 'stroke: #7c3aed; stroke-width: 3; stroke-dasharray: 3 6;',
                    zIndex: 1,
                    label: `hook: ${hookTarget}`,
                    labelStyle: 'fill: #5b21b6; font-weight: 600;',
                  });
                }
              }
            }
          }
        }
      }

      nodes = ns;
      edges = es;
      calculatePositions();
    } catch (error) {
      console.error('Error reading file', error);
    }
  })();

  function calculatePositions() {
    const rootNodes = nodes.filter((n) => n.class?.includes('root') && !n.hidden);
    const dependencyGraph = new Map<string, Set<string>>();
    const incomingCount = new Map<string, number>();

    for (const node of rootNodes) {
      dependencyGraph.set(node.id, new Set());
      incomingCount.set(node.id, 0);
    }

    for (const edge of edges) {
      // Extract root law IDs from nested node IDs
      const sourceRoot = edge.source.split('-')[0];
      const targetRoot = edge.target.split('-')[0];

      if (sourceRoot !== targetRoot) {
        if (!dependencyGraph.has(sourceRoot)) {
          dependencyGraph.set(sourceRoot, new Set());
          incomingCount.set(sourceRoot, 0);
        }
        if (!dependencyGraph.has(targetRoot)) {
          dependencyGraph.set(targetRoot, new Set());
          incomingCount.set(targetRoot, 0);
        }

        if (!dependencyGraph.get(targetRoot)!.has(sourceRoot)) {
          dependencyGraph.get(targetRoot)!.add(sourceRoot);
          incomingCount.set(sourceRoot, (incomingCount.get(sourceRoot) || 0) + 1);
        }
      }
    }

    // Topological sort
    const layers: string[][] = [];
    const processed = new Set<string>();

    let currentLayer = rootNodes
      .map((n) => n.id)
      .filter((id) => (incomingCount.get(id) || 0) === 0);

    while (currentLayer.length > 0) {
      layers.push(currentLayer);
      for (const nodeId of currentLayer) processed.add(nodeId);

      const nextLayer = new Set<string>();
      for (const nodeId of currentLayer) {
        for (const dependent of dependencyGraph.get(nodeId) || new Set()) {
          if (processed.has(dependent)) continue;
          let allDepsProcessed = true;
          for (const edge of edges) {
            const sr = edge.source.split('-')[0];
            const tr = edge.target.split('-')[0];
            if (sr === dependent && tr !== sr && !processed.has(tr)) {
              allDepsProcessed = false;
              break;
            }
          }
          if (allDepsProcessed) nextLayer.add(dependent);
        }
      }
      currentLayer = Array.from(nextLayer);
    }

    const unprocessed = rootNodes.map((n) => n.id).filter((id) => !processed.has(id));
    if (unprocessed.length > 0) layers.push(unprocessed);

    const nodeSpacing = 580;
    const layerSpacing = 100;
    const maxNodesPerColumn = 4;
    const maxColumnPerLayer = new Map<number, number>();

    for (let l = 0; l < layers.length; l++) {
      const layer = layers[l];
      let visibleNodes = layer
        .map((nodeId) => ({ nodeId, nodeIndex: nodes.findIndex((n) => n.id === nodeId) }))
        .filter(({ nodeIndex }) => nodeIndex !== -1 && !nodes[nodeIndex].hidden);

      let columnIndex = 0;
      let y = 0;
      let nodesInCurrentColumn = 0;

      for (const { nodeId, nodeIndex } of visibleNodes) {
        if (nodesInCurrentColumn >= maxNodesPerColumn) {
          columnIndex++;
          y = 0;
          nodesInCurrentColumn = 0;
        }

        let xOffset = 0;
        for (let prevLayer = 0; prevLayer < l; prevLayer++) {
          xOffset += ((maxColumnPerLayer.get(prevLayer) || 0) + 1) * nodeSpacing;
        }

        nodes[nodeIndex] = { ...nodes[nodeIndex], position: { x: xOffset + columnIndex * nodeSpacing, y } };
        y += (nodes[nodeIndex].height || 0) + layerSpacing;
        nodesInCurrentColumn++;
      }

      maxColumnPerLayer.set(l, columnIndex);
    }

    nodes = [...nodes];
  }

  function updateEdgeHighlighting(rootNodeId: string | null) {
    edges = edges.map((edge) => {
      let edgeClass = typeof edge.class === 'string' ? edge.class : '';
      edgeClass = edgeClass.replace(/\b(inbound|outbound)\b/g, '').trim();

      if (rootNodeId) {
        const sourceRoot = edge.source.split('-')[0];
        const targetRoot = edge.target.split('-')[0];
        if (sourceRoot === rootNodeId) {
          edgeClass = edgeClass ? `${edgeClass} inbound` : 'inbound';
        } else if (targetRoot === rootNodeId) {
          edgeClass = edgeClass ? `${edgeClass} outbound` : 'outbound';
        }
      }

      return { ...edge, class: edgeClass || undefined };
    });
  }

  function handleNodeClick({ node, event }: any) {
    if ((event.target as HTMLElement).closest('.close')) {
      nodes = nodes.map((n) => n.id.startsWith(node.id) ? { ...n, hidden: true } : n);
      edges = edges.map((e) =>
        e.source.startsWith(node.id) || e.target.startsWith(node.id) ? { ...e, hidden: true } : e,
      );
      selectedLaws = selectedLaws.filter((id) => id !== node.id);
      if (selectedRootNode === node.id) {
        selectedRootNode = null;
        updateEdgeHighlighting(null);
      }
    } else if (node.class?.includes('root')) {
      if (selectedRootNode === node.id) {
        selectedRootNode = null;
        updateEdgeHighlighting(null);
      } else {
        selectedRootNode = node.id;
        updateEdgeHighlighting(node.id);
      }
    }
  }

  function getLawId(nodeOrEdgeId: string): string {
    // Law IDs use underscores; the first hyphen separates law ID from node type
    const idx = nodeOrEdgeId.indexOf('-');
    return idx === -1 ? nodeOrEdgeId : nodeOrEdgeId.substring(0, idx);
  }

  $effect(() => {
    // Access selectedLaws to register as dependency
    const selected = new Set(selectedLaws);

    nodes = untrack(() => nodes).map((node) => ({
      ...node,
      hidden: !selected.has(getLawId(node.id)),
    }));

    edges = untrack(() => edges).map((edge) => ({
      ...edge,
      hidden: !selected.has(getLawId(edge.source)) || !selected.has(getLawId(edge.target)),
    }));
  });

  // --- Scenario runner wiring ---

  async function loadDependency(lawId: string): Promise<void> {
    const engine = await initEngine();
    if (engine.hasLaw(lawId)) return;
    const yaml = rawYamlById.get(lawId);
    if (!yaml) throw new Error(`Law not cached in demo: ${lawId}`);
    engine.loadLaw(yaml);
  }

  async function loadFeatureList() {
    try {
      const res = await fetch('/features/list');
      if (!res.ok) return;
      features = await res.json();
    } catch (e) {
      console.error('Failed to load features', e);
    }
  }

  async function loadSelectedFeature(name: string) {
    try {
      const res = await fetch(`/feature/${encodeURIComponent(name)}`);
      if (!res.ok) throw new Error(`Failed to load feature: ${res.status}`);
      const text = await res.text();
      featureScenarios = parseFeature(text);
      selectedScenarioIdx = 0;
      resetTrace();
    } catch (e: any) {
      runError = e.message ?? String(e);
    }
  }

  function stripTraceClass(cls: unknown): string {
    if (typeof cls !== 'string') return '';
    return cls
      .replace(/\btrace-active\b/g, '')
      .replace(/\btrace-visited\b/g, '')
      .replace(/\btrace-start\b/g, '')
      .replace(/\s+/g, ' ')
      .trim();
  }

  function clearTraceClasses() {
    edges = untrack(() => edges).map((e) => {
      const cleaned = stripTraceClass(e.class);
      return { ...e, class: cleaned || undefined };
    });
    nodes = untrack(() => nodes).map((n) => {
      const cleaned = stripTraceClass(n.class);
      return { ...n, class: cleaned || undefined };
    });
  }

  function resetTrace() {
    traceRun = null;
    traceSteps = [];
    currentStepIdx = -1;
    runError = null;
    traceStartNodeIds = [];
    clearTraceClasses();
  }

  async function runSelected() {
    if (running) return;
    const scn = featureScenarios[selectedScenarioIdx];
    if (!scn) return;
    running = true;
    runError = null;
    try {
      const engine = await initEngine();
      // Preload all cached laws so cross-law resolution works. loadLaw
      // silently replaces an existing law with the same (id, valid_from).
      for (const [, yaml] of rawYamlById) {
        try {
          engine.loadLaw(yaml);
        } catch {
          // ignore parse errors on unrelated demo laws
        }
      }
      const result = await runScenario(engine, scn, loadDependency);
      traceRun = result;
      const flat = result.trace.trace
        ? flattenTraceSteps(result.trace.trace, scn.lawId)
        : [];
      const currentEdges = untrack(() => edges);
      const currentNodes = untrack(() => nodes);
      traceSteps = flat.map((s) => ({
        ...s,
        edgeIds: edgeIdsForStep(s, currentEdges as any),
        nodeIds: graphNodeIdsForStep(s, currentNodes as any),
      }));

      // Start point: the scenario's root law + initial output leaf.
      const nodeIdSet = new Set(currentNodes.map((n) => n.id));
      const startIds: string[] = [];
      if (nodeIdSet.has(scn.lawId)) startIds.push(scn.lawId);
      const outId = `${scn.lawId}-output-${scn.outputName}`;
      if (nodeIdSet.has(outId)) startIds.push(outId);
      traceStartNodeIds = startIds;
      currentStepIdx = traceSteps.length > 0 ? 0 : -1;
      selectedRootNode = null;
      updateEdgeHighlighting(null);
    } catch (e: any) {
      runError = e.message ?? String(e);
      traceRun = null;
      traceSteps = [];
      currentStepIdx = -1;
    } finally {
      running = false;
    }
  }

  function formatValue(v: unknown): string {
    if (v === null || v === undefined) return '∅';
    if (typeof v === 'string') return v.length > 60 ? v.substring(0, 57) + '…' : v;
    if (typeof v === 'boolean') return v ? 'true' : 'false';
    if (typeof v === 'number') return String(v);
    try {
      const s = JSON.stringify(v);
      return s.length > 80 ? s.substring(0, 77) + '…' : s;
    } catch {
      return String(v);
    }
  }

  function stepPrev() {
    if (currentStepIdx > 0) currentStepIdx -= 1;
  }

  function stepNext() {
    if (currentStepIdx < traceSteps.length - 1) currentStepIdx += 1;
  }

  // Update trace-active / trace-visited / trace-start classes when the
  // current step changes. trace-start is sticky across all steps.
  $effect(() => {
    if (traceSteps.length === 0 || currentStepIdx < 0) return;

    const activeStep = traceSteps[currentStepIdx];
    const activeEdgeIds = new Set(activeStep?.edgeIds ?? []);
    const activeNodeIds = new Set(activeStep?.nodeIds ?? []);
    const visitedEdgeIds = new Set<string>();
    const visitedNodeIds = new Set<string>();
    for (let i = 0; i < currentStepIdx; i++) {
      for (const eid of traceSteps[i].edgeIds) visitedEdgeIds.add(eid);
      for (const nid of traceSteps[i].nodeIds) visitedNodeIds.add(nid);
    }
    const startIds = new Set(traceStartNodeIds);

    edges = untrack(() => edges).map((e) => {
      const base = stripTraceClass(e.class);
      let cls = base;
      if (activeEdgeIds.has(e.id)) {
        cls = cls ? `${cls} trace-active` : 'trace-active';
      } else if (visitedEdgeIds.has(e.id)) {
        cls = cls ? `${cls} trace-visited` : 'trace-visited';
      }
      return { ...e, class: cls || undefined };
    });

    nodes = untrack(() => nodes).map((n) => {
      const base = stripTraceClass(n.class);
      const classes: string[] = [];
      if (base) classes.push(base);
      if (startIds.has(n.id)) classes.push('trace-start');
      if (activeNodeIds.has(n.id)) classes.push('trace-active');
      else if (visitedNodeIds.has(n.id)) classes.push('trace-visited');
      return { ...n, class: classes.join(' ') || undefined };
    });
  });

  // Load features once on mount
  $effect(() => {
    loadFeatureList();
  });

  // Load a feature's scenarios when selected
  $effect(() => {
    if (selectedFeature) {
      loadSelectedFeature(selectedFeature);
    }
  });

  const layerLabels: Record<string, string> = {
    WET: 'Wet',
    AMVB: 'AMvB',
    MINISTERIELE_REGELING: 'Ministeriële regeling',
    GEMEENTELIJKE_VERORDENING: 'Gemeentelijke verordening',
    GRONDWET: 'Grondwet',
  };
</script>

<svelte:head>
  <title>Wettengraaf — Regelrecht</title>
</svelte:head>

<div class="float-right h-screen w-80 overflow-y-auto px-6 pb-4 text-sm">
  <div class="sticky top-0 bg-white pt-6 pb-2">
    <h1 class="mb-3 text-base font-semibold">Selectie van wetten</h1>

    <div class="flex gap-2">
      <button
        type="button"
        onclick={calculatePositions}
        class="cursor-pointer rounded-md border border-blue-600 bg-blue-600 px-3 py-1.5 text-white transition duration-200 hover:border-blue-700 hover:bg-blue-700"
        >Her-positioneer</button
      >
      <button
        type="button"
        onclick={() => { selectedLaws = laws.filter(l => l.articles.length > 0).map((law) => law.id); }}
        class="cursor-pointer rounded-md border border-gray-600 bg-gray-600 px-3 py-1.5 text-white transition duration-200 hover:border-gray-700 hover:bg-gray-700"
        >Selecteer alles</button
      >
    </div>
  </div>

  {#each Object.entries(laws.filter(l => l.articles.length > 0).reduce((acc, law) => {
        if (!acc[law.layer]) acc[law.layer] = [];
        acc[law.layer].push(law);
        return acc;
      }, {} as Record<string, Law[]>)) as [layer, layerLaws]}
    <h2
      class="service-{getLayerColorIndex(layer)} mt-4 mb-2 inline-block rounded-md px-2 py-1 text-sm font-semibold first:mt-0"
    >
      {layerLabels[layer] || layer}
    </h2>
    {#each layerLaws as law}
      <div class="mb-1.5">
        <label class="group inline-flex items-start">
          <input
            bind:group={selectedLaws}
            class="form-checkbox mt-0.5 mr-1.5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
            type="checkbox"
            value={law.id}
          />
          <span
            >{law.name}
            <button
              type="button"
              onclick={() => { selectedLaws = [law.id]; }}
              class="invisible cursor-pointer font-semibold text-blue-700 group-hover:visible hover:text-blue-800"
              >alleen</button
            ></span
          >
        </label>
      </div>
    {/each}
  {/each}
</div>

<div class="mr-80 flex h-screen flex-col">
  <!-- Scenario toolbar -->
  <div class="flex items-center gap-2 border-b border-gray-200 bg-white px-4 py-2 text-sm">
    <label class="font-semibold text-gray-700" for="feature-select">Feature:</label>
    <select
      id="feature-select"
      bind:value={selectedFeature}
      class="rounded border border-gray-300 px-2 py-1 text-sm"
    >
      <option value={null}>— kies een feature —</option>
      {#each features as f}
        <option value={f}>{f}</option>
      {/each}
    </select>

    <label class="ml-4 font-semibold text-gray-700" for="scenario-select">Scenario:</label>
    <select
      id="scenario-select"
      bind:value={selectedScenarioIdx}
      disabled={featureScenarios.length === 0}
      class="max-w-lg flex-1 rounded border border-gray-300 px-2 py-1 text-sm disabled:opacity-50"
    >
      {#each featureScenarios as scn, i}
        <option value={i}>{scn.name}</option>
      {/each}
    </select>

    <button
      type="button"
      onclick={runSelected}
      disabled={running || featureScenarios.length === 0 || !featureScenarios[selectedScenarioIdx]?.lawId}
      class="ml-2 cursor-pointer rounded-md border border-emerald-600 bg-emerald-600 px-3 py-1 text-white transition hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50"
    >
      {running ? 'Bezig…' : '▶ Run'}
    </button>

    {#if traceRun}
      <button
        type="button"
        onclick={resetTrace}
        class="cursor-pointer rounded-md border border-gray-400 px-3 py-1 text-gray-700 transition hover:bg-gray-100"
      >
        Reset
      </button>
    {/if}

    {#if runError}
      <span class="ml-2 truncate text-red-700" title={runError}>⚠ {runError}</span>
    {:else if traceRun}
      {@const passed = traceRun.assertions.filter((a) => a.passed).length}
      {@const total = traceRun.assertions.length}
      <span
        class="ml-2 rounded px-2 py-0.5 text-xs font-semibold {passed === total
          ? 'bg-emerald-100 text-emerald-800'
          : 'bg-red-100 text-red-800'}"
      >
        {passed}/{total} asserts
      </span>
    {/if}
  </div>

  <!-- Graph -->
  <div class="min-h-0 flex-1">
    <SvelteFlow
      bind:nodes
      bind:edges
      {nodeTypes}
      onnodeclick={handleNodeClick}
      fitView
      nodesConnectable={false}
      proOptions={{ hideAttribution: true }}
      minZoom={0.1}
    >
      <Controls showLock={false} />
      <Background variant={BackgroundVariant.Dots} />
      <MiniMap
        zoomable
        pannable
        nodeColor={(n) => (n.class?.includes('root') && !n.hidden ? '#ccc' : 'transparent')}
      />
    </SvelteFlow>
  </div>

  <!-- Trace panel -->
  {#if traceRun && traceSteps.length > 0}
    {@const visibleSteps = traceFilter === 'highlights'
      ? traceSteps.filter((s) => s.edgeIds.length > 0 || s.nodeIds.length > 0)
      : traceSteps}
    <div class="flex h-[40vh] flex-col border-t-2 border-amber-500 bg-white text-sm">
      <div class="flex flex-wrap items-center gap-2 border-b border-gray-200 bg-amber-50 px-4 py-2">
        <button
          type="button"
          onclick={stepPrev}
          disabled={currentStepIdx <= 0}
          class="cursor-pointer rounded border border-gray-400 px-2 py-0.5 hover:bg-white disabled:cursor-not-allowed disabled:opacity-40"
          >◀ Prev</button
        >
        <button
          type="button"
          onclick={stepNext}
          disabled={currentStepIdx >= traceSteps.length - 1}
          class="cursor-pointer rounded border border-gray-400 px-2 py-0.5 hover:bg-white disabled:cursor-not-allowed disabled:opacity-40"
          >Next ▶</button
        >
        <span class="text-gray-600">
          Step <strong>{currentStepIdx + 1}</strong> / {traceSteps.length}
        </span>

        <div class="ml-auto flex items-center gap-1 text-xs">
          <span class="text-gray-500">Filter:</span>
          <button
            type="button"
            onclick={() => (traceFilter = 'highlights')}
            class="cursor-pointer rounded border px-2 py-0.5 {traceFilter === 'highlights' ? 'border-amber-500 bg-amber-100 font-semibold' : 'border-gray-300 bg-white'}"
            >Met highlights</button
          >
          <button
            type="button"
            onclick={() => (traceFilter = 'all')}
            class="cursor-pointer rounded border px-2 py-0.5 {traceFilter === 'all' ? 'border-amber-500 bg-amber-100 font-semibold' : 'border-gray-300 bg-white'}"
            >Alles ({traceSteps.length})</button
          >

          <span class="ml-3 text-gray-500">Weergave:</span>
          <button
            type="button"
            onclick={() => (traceView = 'steps')}
            class="cursor-pointer rounded border px-2 py-0.5 {traceView === 'steps' ? 'border-amber-500 bg-amber-100 font-semibold' : 'border-gray-300 bg-white'}"
            >Stappen</button
          >
          <button
            type="button"
            onclick={() => (traceView = 'tree')}
            class="cursor-pointer rounded border px-2 py-0.5 {traceView === 'tree' ? 'border-amber-500 bg-amber-100 font-semibold' : 'border-gray-300 bg-white'}"
            >Boom</button
          >
        </div>
      </div>

      <div class="flex min-h-0 flex-1">
        <!-- Left: step list OR box-drawing tree -->
        <div class="w-1/2 overflow-y-auto border-r border-gray-200">
          {#if traceView === 'tree'}
            <pre class="whitespace-pre p-3 font-mono text-[11px] leading-4 text-gray-800">{traceRun.trace.trace_text ?? '(geen box-drawing trace)'}</pre>
          {:else}
            {#each visibleSteps as step}
              {@const i = traceSteps.indexOf(step)}
              <button
                type="button"
                onclick={() => (currentStepIdx = i)}
                class="group block w-full cursor-pointer border-b border-gray-100 px-3 py-1 text-left font-mono text-xs hover:bg-gray-50 {i ===
                currentStepIdx
                  ? 'bg-amber-100 font-semibold text-amber-900'
                  : ''}"
                style="padding-left: {step.depth * 10 + 12}px"
              >
                <div class="flex items-baseline gap-1 truncate">
                  <span class="inline-block w-7 text-right text-gray-400">{i + 1}.</span>
                  <span class="node-type-{step.nodeType} rounded px-1 text-[10px] uppercase tracking-wide">
                    {step.nodeType.replace(/_/g, ' ')}
                  </span>
                  <span class="truncate">{step.name}</span>
                  {#if step.resolveType}
                    <span class="text-[10px] text-indigo-600">[{step.resolveType}]</span>
                  {/if}
                </div>
                {#if step.result !== undefined && step.result !== null}
                  <div class="pl-8 text-[11px] text-emerald-700 truncate">= {formatValue(step.result)}</div>
                {/if}
                {#if step.message}
                  <div class="pl-8 text-[10px] italic text-gray-500 truncate">{step.message}</div>
                {/if}
                {#if step.edgeIds.length > 0 || step.nodeIds.length > 0}
                  <div class="pl-8 text-[10px] text-amber-600">
                    {#if step.edgeIds.length > 0}→ {step.edgeIds.length} edge{step.edgeIds.length === 1 ? '' : 's'}{/if}
                    {#if step.nodeIds.length > 0}{step.edgeIds.length > 0 ? ', ' : '→ '}{step.nodeIds.length} node{step.nodeIds.length === 1 ? '' : 's'}{/if}
                  </div>
                {/if}
              </button>
            {/each}
          {/if}
        </div>

        <!-- Right: current step detail + outputs + assertions -->
        <div class="w-1/2 overflow-y-auto bg-gray-50 p-3">
          {#if currentStepIdx >= 0 && traceSteps[currentStepIdx]}
            {@const cur = traceSteps[currentStepIdx]}
            <div class="mb-3 rounded border border-amber-300 bg-amber-50 p-2">
              <div class="mb-1 flex items-center gap-2">
                <span class="node-type-{cur.nodeType} rounded px-1 text-[10px] uppercase tracking-wide">{cur.nodeType.replace(/_/g, ' ')}</span>
                <span class="font-mono text-xs font-semibold">{cur.name}</span>
              </div>
              <dl class="space-y-0.5 font-mono text-[11px]">
                <div class="flex gap-2">
                  <dt class="w-20 text-gray-500">Wet:</dt>
                  <dd class="text-gray-900">{cur.lawId}</dd>
                </div>
                {#if cur.resolveType}
                  <div class="flex gap-2">
                    <dt class="w-20 text-gray-500">Resolve:</dt>
                    <dd class="text-indigo-700">{cur.resolveType}</dd>
                  </div>
                {/if}
                {#if cur.result !== undefined && cur.result !== null}
                  <div class="flex gap-2">
                    <dt class="w-20 text-gray-500">Resultaat:</dt>
                    <dd class="text-emerald-700">{formatValue(cur.result)}</dd>
                  </div>
                {/if}
                {#if cur.durationUs !== undefined}
                  <div class="flex gap-2">
                    <dt class="w-20 text-gray-500">Duur:</dt>
                    <dd class="text-gray-700">{cur.durationUs}µs</dd>
                  </div>
                {/if}
                {#if cur.message}
                  <div class="flex gap-2">
                    <dt class="w-20 text-gray-500">Bericht:</dt>
                    <dd class="italic text-gray-700">{cur.message}</dd>
                  </div>
                {/if}
              </dl>
            </div>
          {/if}

          <h3 class="mb-1 font-semibold text-gray-700">Outputs</h3>
          <dl class="mb-3 space-y-0.5 font-mono text-xs">
            {#each Object.entries(traceRun.trace.outputs ?? {}) as [k, v]}
              <div class="flex gap-2">
                <dt class="text-gray-500">{k}:</dt>
                <dd class="text-gray-900">{formatValue(v)}</dd>
              </div>
            {/each}
          </dl>

          {#if traceRun.assertions.length > 0}
            <h3 class="mb-1 font-semibold text-gray-700">Assertions</h3>
            <ul class="space-y-0.5 font-mono text-xs">
              {#each traceRun.assertions as a}
                <li class="flex gap-2">
                  <span class={a.passed ? 'text-emerald-600' : 'text-red-600'}>
                    {a.passed ? '✓' : '✗'}
                  </span>
                  <span class="text-gray-700">
                    {a.output} = {a.expected}
                    {#if !a.passed}
                      <span class="text-red-600">(got {formatValue(a.actual)})</span>
                    {/if}
                  </span>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

<style lang="postcss">
  @reference "tailwindcss/theme";

  :global(.root) { @apply rounded-md border border-black p-2; }

  :global(.service-0.root) { @apply border-blue-800 bg-blue-50; }
  :global(.service-0.property-group) { @apply border-blue-800 bg-blue-100; }
  :global(.service-1.root) { @apply border-pink-800 bg-pink-50; }
  :global(.service-1.property-group) { @apply border-pink-800 bg-pink-100; }
  :global(.service-2.root) { @apply border-emerald-800 bg-emerald-50; }
  :global(.service-2.property-group) { @apply border-emerald-800 bg-emerald-100; }
  :global(.service-3.root) { @apply border-amber-800 bg-amber-50; }
  :global(.service-3.property-group) { @apply border-amber-800 bg-amber-100; }
  :global(.service-4.root) { @apply border-purple-800 bg-purple-50; }
  :global(.service-4.property-group) { @apply border-purple-800 bg-purple-100; }
  :global(.service-5.root) { @apply border-yellow-800 bg-yellow-50; }
  :global(.service-5.property-group) { @apply border-yellow-800 bg-yellow-100; }
  :global(.service-6.root) { @apply border-slate-800 bg-slate-50; }
  :global(.service-6.property-group) { @apply border-slate-800 bg-slate-100; }

  .service-0 { @apply bg-blue-100 text-blue-800; }
  .service-1 { @apply bg-pink-100 text-pink-800; }
  .service-2 { @apply bg-emerald-100 text-emerald-800; }
  .service-3 { @apply bg-amber-100 text-amber-800; }
  .service-4 { @apply bg-purple-100 text-purple-800; }
  .service-5 { @apply bg-yellow-100 text-yellow-800; }
  .service-6 { @apply bg-slate-100 text-slate-800; }

  :global(.property-group, .svelte-flow__node-input, .svelte-flow__node-source, .svelte-flow__node-output) {
    @apply cursor-grab overflow-hidden text-ellipsis;
  }

  :global(.svelte-flow) {
    --xy-edge-stroke: #3b82f6;
    --xy-edge-stroke-selected: #1d4ed8;
    --xy-edge-stroke-width-default: 3;
  }
  :global(.svelte-flow__arrowhead polyline) { @apply !fill-sky-500 !stroke-sky-500; }
  :global(.svelte-flow__edge.selected) { --xy-edge-stroke-width-default: 5; }

  /* Click-highlight edges */
  :global(.svelte-flow__edge.inbound) { --xy-edge-stroke: #ef4444; --xy-edge-stroke-width-default: 5; }
  :global(.svelte-flow__edge.outbound) { --xy-edge-stroke: #22c55e; --xy-edge-stroke-width-default: 5; }
  :global(.svelte-flow__edge.inbound path:first-child, .svelte-flow__edge.outbound path:first-child) {
    marker-end: none;
  }

  /* Trace stepping highlights — use !important to override inline stroke
     styles that hook/impl/ovr edges set in +page.svelte:587,613,670. */
  :global(.svelte-flow__edge.trace-active path) {
    stroke: #f59e0b !important;
    stroke-dasharray: none !important;
    animation: trace-pulse 1.2s ease-in-out infinite;
  }
  :global(.svelte-flow__edge.trace-visited path) {
    stroke: #fbbf24 !important;
    stroke-width: 4 !important;
    opacity: 0.75;
  }
  @keyframes trace-pulse {
    0%, 100% { stroke-width: 6; }
    50% { stroke-width: 10; }
  }

  /* Trace-highlight for graph nodes (leaves + root law cards).
     Leaves are <div>s with tailwind padding — we override background
     directly so it's unmistakable against the already-tinted property
     groups. Root law cards get an outline + glow instead. */
  :global(.svelte-flow__node.trace-active) {
    z-index: 10;
  }
  :global(.svelte-flow__node-leaf.trace-active) {
    background: #f59e0b !important;
    color: #1f2937 !important;
    font-weight: 700 !important;
    outline: 3px solid #b45309 !important;
    outline-offset: 0 !important;
    box-shadow: 0 0 20px rgba(245, 158, 11, 0.8) !important;
  }
  :global(.svelte-flow__node-law.trace-active),
  :global(.svelte-flow__node-default.trace-active) {
    outline: 4px solid #f59e0b !important;
    outline-offset: 2px !important;
    box-shadow: 0 0 24px rgba(245, 158, 11, 0.75) !important;
  }

  :global(.svelte-flow__node-leaf.trace-visited) {
    background: #fde68a !important;
    color: #78350f !important;
  }
  :global(.svelte-flow__node-law.trace-visited),
  :global(.svelte-flow__node-default.trace-visited) {
    outline: 2px solid #fbbf24 !important;
    outline-offset: 1px !important;
  }

  /* Start point — sticky blue outline on the scenario's entry law + output */
  :global(.svelte-flow__node.trace-start) {
    outline: 3px dashed #2563eb;
    outline-offset: 4px;
    z-index: 9;
  }
  :global(.svelte-flow__node.trace-start::before) {
    content: '▶ start';
    position: absolute;
    top: -18px;
    left: -4px;
    background: #2563eb;
    color: white;
    font-size: 10px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 3px;
    letter-spacing: 0.04em;
    pointer-events: none;
    z-index: 11;
  }
  /* When a start node is also active, combine outlines cleanly */
  :global(.svelte-flow__node.trace-start.trace-active) {
    outline: 3px solid #f59e0b;
    box-shadow: 0 0 0 7px rgba(37, 99, 235, 0.35), 0 0 16px rgba(245, 158, 11, 0.55);
  }

  /* Trace step type chips */
  .node-type-article { @apply bg-blue-200 text-blue-900; }
  .node-type-action { @apply bg-emerald-200 text-emerald-900; }
  .node-type-requirement { @apply bg-sky-200 text-sky-900; }
  .node-type-resolve { @apply bg-slate-200 text-slate-800; }
  .node-type-operation { @apply bg-gray-200 text-gray-800; }
  .node-type-cached { @apply bg-gray-100 text-gray-600; }
  .node-type-cross_law_reference { @apply bg-amber-200 text-amber-900; }
  .node-type-open_term_resolution { @apply bg-green-200 text-green-900; }
  .node-type-hook_resolution { @apply bg-purple-200 text-purple-900; }
  .node-type-override_resolution { @apply bg-red-200 text-red-900; }
</style>
