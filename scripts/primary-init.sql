-- Enable replication for the postgres user
ALTER USER postgres WITH REPLICATION;

-- Create a replication slot (optional but recommended)
SELECT pg_create_physical_replication_slot('replica_slot');