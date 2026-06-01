using System;
using System.Collections.Generic;

public class GraphWalker
{
    private List<string> nodes;

    public GraphWalker(List<string> nodes)
    {
        this.nodes = nodes;
    }

    public Dictionary<string, object> buildMiniGraph(int count)
    {
        var result = new Dictionary<string, object>();
        foreach (var n in nodes)
        {
            result[n] = count;
        }
        return result;
    }

    public static GraphWalker lazyBuildContext(string[] items)
    {
        return new GraphWalker(new List<string>(items));
    }
}
