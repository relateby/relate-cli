/**
 * get_colleagues
 *
 * Find people who work at the same company as the given person.
 * Results are ordered by colleague name.
 *
 * @param {string} name - Full name of the person
 * @param {integer} [limit=25] - Maximum number of results to return
 * @returns {[colleague_name: string, company: string][]} - One row per colleague found
 */
MATCH (p:Person {name: $name})-[:WORKS_AT]->(c:Company)<-[:WORKS_AT]-(colleague:Person)
RETURN colleague.name AS colleague_name, c.name AS company
ORDER BY colleague_name
LIMIT $limit
