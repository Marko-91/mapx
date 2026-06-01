class GraphWalker(private val nodes: List<String>) {
    fun buildMiniGraph(count: Int): Map<String, Any> {
        val result = mutableMapOf<String, Any>()
        for (n in nodes) {
            result[n] = count
        }
        return result
    }
}

fun lazyBuildContext(items: Array<String>): GraphWalker {
    return GraphWalker(items.toList())
}

fun main() {
    val walker = GraphWalker(listOf("a", "b"))
    val graph = walker.buildMiniGraph(2)
    println(graph)
}
