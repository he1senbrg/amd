-- Enable the pgcrypto extension for UUID generation
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Create the Member table
CREATE TABLE Member (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    discord_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT now()
);

-- Create the Update table, using "Update" with quotes because it's a reserved keyword
CREATE TABLE "Update" (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    member_id UUID NOT NULL,
    message_date TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT now(),
    FOREIGN KEY (member_id) REFERENCES Member(id)
);
