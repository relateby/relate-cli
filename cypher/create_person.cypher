/**
 * create_person
 *
 * Create a new Person node. Fails if a person with the same name already exists
 * and a uniqueness constraint exists on `name`.
 *
 * @param {string} name - The person's full name
 * @param {integer} age - Age in years
 * @param {string} [email=""] - Email address
 * @returns {[person: node<Person>]} - The created person node
 */
CREATE (person:Person {name: $name, age: $age, email: $email})
RETURN person
