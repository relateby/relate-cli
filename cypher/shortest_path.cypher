/**
 * find_shortest_path
 *
 * Find the shortest connection between two people in the social graph.
 * Returns no rows if no connection exists.
 *
 * @param {string} from_name - Name of the starting person
 * @param {string} to_name - Name of the target person
 * @returns {[connection: path, hops: integer]} - Shortest path if one exists
 */
MATCH path = shortestPath(
  (a:Person {name: $from_name})-[*]-(b:Person {name: $to_name})
)
RETURN path AS connection, length(path) AS hops
