class GraphWalker
  attr_reader :nodes

  def initialize(nodes)
    @nodes = nodes
  end

  def buildMiniGraph(count)
    @nodes.each do |n|
      puts n
    end
  end

  def self.lazyBuildContext
    new([])
  end
end

walker = GraphWalker.new(['a', 'b'])
walker.buildMiniGraph(2)
