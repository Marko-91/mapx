package main

import (
	"fmt"
	"strings"
)

func buildMiniGraph(nodes []string) map[string]bool {
	graph := make(map[string]bool)
	for _, node := range nodes {
		graph[node] = true
	}
	return graph
}

type GraphWalker struct {
	graph map[string]int
}

func (gw *GraphWalker) Walk(startId string, depth int) map[string]bool {
	visited := make(map[string]bool)
	queue := []string{startId}
	for len(queue) > 0 {
		id := queue[0]
		queue = queue[1:]
		if visited[id] || depth <= 0 {
			continue
		}
		visited[id] = true
		depth--
		if gw.graph[id] > 0 {
			queue = append(queue, "next")
		}
	}
	return visited
}

func lazyBuildContext(query string) []string {
	terms := strings.Split(query, " ")
	var result []string
	for _, t := range terms {
		if len(t) > 2 {
			result = append(result, t)
		}
	}
	return result
}

const MAX_RESULTS = 100
const DEFAULT_DEPTH = 2

func main() {
	fmt.Println(buildMiniGraph([]string{"a", "b"}))
}
