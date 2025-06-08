CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";
DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1 FROM pg_proc
            WHERE proname = 'uuidv7'
              AND pg_function_is_visible(oid)
        ) THEN
            CREATE EXTENSION IF NOT EXISTS "pg_uuidv7";

            IF NOT EXISTS (
                SELECT 1 FROM pg_proc
                WHERE proname = 'uuidv7'
                  AND pg_function_is_visible(oid)
            ) THEN
                EXECUTE 'CREATE OR REPLACE FUNCTION uuidv7() RETURNS UUID AS $func$ BEGIN RETURN uuid_generate_v7(); END; $func$ LANGUAGE plpgsql VOLATILE;';
            END IF;
        END IF;
    END
$$;
SELECT uuidv7() AS test_uuid_v7;
