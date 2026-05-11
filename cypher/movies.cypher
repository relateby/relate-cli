/**
 * create
 *
 * Create a movie node. Fails if a movie with the same title already exists.
 *
 * @param {string} title - Title of the movie
 * @param {integer} released - Year the movie was released
 * @param {string} [tagline=""] - Promotional tagline
 * @returns {[movie: node<Movie>]} - The created node
 */
CREATE (m:Movie {title: $title, released: $released, tagline: $tagline})
RETURN m;

/**
 * upsert
 *
 * Create or update a movie node, matched by title.
 *
 * @param {string} title - Title of the movie (used as key)
 * @param {integer} released - Year the movie was released
 * @param {string} [tagline=""] - Promotional tagline
 * @returns {[movie: node<Movie>]} - The upserted node
 */
MERGE (m:Movie {title: $title})
SET m.released = $released, m.tagline = $tagline
RETURN m;

/**
 * find_by_title
 *
 * Find a movie by its exact title.
 *
 * @param {string} title - Exact title to match
 * @returns {[movie: node<Movie>][]} - Matching movies (0 or 1)
 */
MATCH (m:Movie {title: $title})
RETURN m;

/**
 * find_by_year
 *
 * Find all movies released in a given year.
 *
 * @param {integer} released - Release year to filter by
 * @returns {[title: string, tagline: string][]} - Titles and taglines
 */
MATCH (m:Movie {released: $released})
RETURN m.title AS title, m.tagline AS tagline
ORDER BY m.title;

/**
 * find_with_actors
 *
 * Find a movie and all actors who appeared in it.
 *
 * @param {string} title - Title of the movie
 * @returns {[movie: string, actor: string, roles: list<string>][]} - Movie, actor name, and roles
 */
MATCH (m:Movie {title: $title})<-[:ACTED_IN]-(p:Person)
RETURN m.title AS movie, p.name AS actor, p.roles AS roles
ORDER BY p.name;

/**
 * update_tagline
 *
 * Update the promotional tagline for a movie.
 *
 * @param {string} title - Title of the movie to update
 * @param {string} tagline - New tagline text
 * @returns {[movie: node<Movie>]} - The updated node
 */
MATCH (m:Movie {title: $title})
SET m.tagline = $tagline
RETURN m;

/**
 * delete
 *
 * Remove a movie node and all its relationships.
 *
 * @param {string} title - Title of the movie to remove
 */
MATCH (m:Movie {title: $title})
DETACH DELETE m
