#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char* id;
    int value;
} GraphWalker;

GraphWalker* buildMiniGraph(char** nodes, int count) {
    GraphWalker* graph = malloc(count * sizeof(GraphWalker));
    for (int i = 0; i < count; i++) {
        graph[i].id = strdup(nodes[i]);
        graph[i].value = 1;
    }
    return graph;
}

void walk(GraphWalker* walker, char* startId, int depth) {
    (void)walker;
    (void)startId;
    (void)depth;
}

char** lazyBuildContext(char* query, int* count) {
    (void)query;
    (void)count;
    return NULL;
}

#define MAX_RESULTS 100
#define DEFAULT_DEPTH 2

int main() {
    char* nodes[] = {"a", "b"};
    int count = sizeof(nodes) / sizeof(nodes[0]);
    GraphWalker* g = buildMiniGraph(nodes, count);
    free(g);
    return 0;
}
