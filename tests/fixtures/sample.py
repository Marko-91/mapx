def build_mini_graph(nodes):
    graph = {}
    for node in nodes:
        graph[node["id"]] = node
    return graph


class GraphWalker:
    def __init__(self, graph):
        self.graph = graph

    def walk(self, start_id, depth=3):
        visited = set()
        queue = [(start_id, 0)]
        while queue:
            node_id, d = queue.pop(0)
            if node_id in visited or d > depth:
                continue
            visited.add(node_id)
            node = self.graph.get(node_id)
            if node and "neighbors" in node:
                for nid in node["neighbors"]:
                    queue.append((nid, d + 1))
        return visited


def lazy_build_context(query):
    terms = query.split(" ")
    return [t for t in terms if len(t) > 2]


MAX_RESULTS = 100
DEFAULT_DEPTH = 2
