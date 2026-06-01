import java.util.*;

public class sample {
    public static Map<String, Boolean> buildMiniGraph(List<String> nodes) {
        Map<String, Boolean> graph = new HashMap<>();
        for (String node : nodes) {
            graph.put(node, true);
        }
        return graph;
    }

    static class GraphWalker {
        private Map<String, Integer> graph;

        public GraphWalker(Map<String, Integer> graph) {
            this.graph = graph;
        }

        public Set<String> walk(String startId, int depth) {
            Set<String> visited = new HashSet<>();
            Queue<String> queue = new LinkedList<>();
            queue.add(startId);
            while (!queue.isEmpty()) {
                String id = queue.poll();
                if (visited.contains(id) || depth <= 0) continue;
                visited.add(id);
                depth--;
                if (graph.containsKey(id) && graph.get(id) > 0) {
                    queue.add("next");
                }
            }
            return visited;
        }
    }

    public static List<String> lazyBuildContext(String query) {
        String[] terms = query.split(" ");
        List<String> result = new ArrayList<>();
        for (String t : terms) {
            if (t.length() > 2) result.add(t);
        }
        return result;
    }

    static final int MAX_RESULTS = 100;
    static final int DEFAULT_DEPTH = 2;
}
