<?php

function buildMiniGraph($nodes) {
    $graph = [];
    foreach ($nodes as $node) {
        $graph[$node['id']] = $node;
    }
    return $graph;
}

class GraphWalker {
    private $graph;

    public function __construct($graph) {
        $this->graph = $graph;
    }

    public function walk($startId, $depth = 3) {
        $visited = [];
        $queue = [['id' => $startId, 'depth' => 0]];
        while (!empty($queue)) {
            $item = array_shift($queue);
            $id = $item['id'];
            $d = $item['depth'];
            if (isset($visited[$id]) || $d > $depth) continue;
            $visited[$id] = true;
            $node = $this->graph[$id] ?? null;
            if ($node && isset($node['neighbors'])) {
                foreach ($node['neighbors'] as $nid) {
                    $queue[] = ['id' => $nid, 'depth' => $d + 1];
                }
            }
        }
        return array_keys($visited);
    }
}

function lazyBuildContext($query) {
    $terms = explode(' ', $query);
    return array_filter($terms, fn($t) => strlen($t) > 2);
}

define('MAX_RESULTS', 100);
define('DEFAULT_DEPTH', 2);
