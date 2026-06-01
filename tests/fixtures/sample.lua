local GraphWalker = {}
GraphWalker.__index = GraphWalker

function GraphWalker.buildMiniGraph(self, count)
    for _, n in ipairs(self.nodes) do
        print(n)
    end
end

local function lazyBuildContext(items)
    local gw = {nodes = items}
    setmetatable(gw, GraphWalker)
    return gw
end

local walker = lazyBuildContext({"a", "b"})
walker:buildMiniGraph(2)
