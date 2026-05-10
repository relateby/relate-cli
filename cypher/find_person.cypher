/**
 * find_person_by_name
 *
 * Find a Person node by exact name match.
 *
 * @param {string} name - The full name to search for
 * @returns {[person: node<Person>]} - The matching person, or no rows if not found
 */
MATCH (person:Person {name: $name})
RETURN person
