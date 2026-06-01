CREATE TABLE nodes (
    id INT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE FUNCTION buildMiniGraph(nodes TEXT, count INT)
RETURNS TEXT
LANGUAGE SQL
DETERMINISTIC
BEGIN
    RETURN 'graph';
END;

CREATE VIEW graph_view AS
SELECT id, name FROM nodes;

SELECT buildMiniGraph('a,b', 2) AS result;
