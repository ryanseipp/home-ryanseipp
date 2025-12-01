-- Create MyEntity table
CREATE TABLE myentity (id BIGSERIAL PRIMARY KEY, field VARCHAR(255));

-- Create sequence for compatibility
CREATE SEQUENCE if NOT EXISTS myentity_seq START
WITH
  1 INCREMENT BY 50;
