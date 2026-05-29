function buildMiniGraph(nodes) {
  const graph = new Map();
  for (const node of nodes) {
    graph.set(node.id, node);
  }
  return graph;
}

class GraphWalker {
  constructor(graph) {
    this.graph = graph;
  }

  walk(startId, depth = 3) {
    const visited = new Set();
    const queue = [{ id: startId, depth: 0 }];
    while (queue.length > 0) {
      const { id, depth: d } = queue.shift();
      if (visited.has(id) || d > depth) continue;
      visited.add(id);
      const node = this.graph.get(id);
      if (node && node.neighbors) {
        for (const nid of node.neighbors) {
          queue.push({ id: nid, depth: d + 1 });
        }
      }
    }
    return visited;
  }
}

function lazyBuildContext(query) {
  const terms = query.split(" ");
  return terms.filter(t => t.length > 2);
}

const MAX_RESULTS = 100;
const DEFAULT_DEPTH = 2;
