ALTER TABLE notes ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'
 CHECK(json_valid(tags) AND json_type(tags) = 'array');
