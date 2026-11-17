-- Usuarios que se autentican en la API (módulo auth).
--
-- - id: UUID v7 (ordenable por fecha de creación). Lo genera la aplicación;
--   el DEFAULT solo cubre inserciones manuales.
-- - email: siempre en minúsculas (lo normaliza el value object Email), así
--   la restricción UNIQUE no distingue mayúsculas. 254 es el máximo del RFC.
-- - password_hash: hash Argon2id en formato PHC. Nunca la contraseña.

CREATE TABLE users (
    id            uuid         PRIMARY KEY DEFAULT uuidv7(),
    email         varchar(254) NOT NULL,
    password_hash text         NOT NULL,
    created_at    timestamptz  NOT NULL DEFAULT now(),
    updated_at    timestamptz  NOT NULL DEFAULT now(),

    CONSTRAINT users_email_key UNIQUE (email),
    CONSTRAINT users_email_lowercase CHECK (email = lower(email))
);

-- Mantiene updated_at al día en cada UPDATE, venga de donde venga.
CREATE FUNCTION set_updated_at() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
