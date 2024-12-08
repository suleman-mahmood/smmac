create type ElementType as enum (
	'SPAN',
	'H_THREE'
);

create table product (
	id uuid primary key,
	niche text not null,
	product text not null,
	domain_boolean_search text not null, -- Rename to url, unique
	created_at timestamptz not null default now()
);

create table domain (
	id uuid primary key,
	product_id uuid not null references product(id),
	domain_candidate_url text not null, -- unique
	domain text, -- duplicates
	founder_boolean_search text, -- Rename to url, duplicates
	created_at timestamptz not null default now()
);

create table founder (
	id uuid primary key,
	domain text, -- unique
	-- domain_id uuid not null references domain(id), -- Remove this
	element_content text not null,
	element_type ElementType not null,
	founder_name text,
	created_at timestamptz not null default now()
);

create table email (
	id uuid primary key,
	founder_id uuid not null references founder(id),
	domain_id uuid not null references domain(id),
	email_address text not null unique,
	verified bool not null,
	created_at timestamptz not null default now()
);
