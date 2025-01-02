create type SmartScoutJobStatus as enum (
  'STARTED',
  'COMPLETED',
  'FAILED'
);

create table smart_scout_job (
  id bigint primary key generated always as identity,
  smart_scout_id bigint not null references smart_scout(id),
  status SmartScoutJobStatus not null,

	created_at timestamptz not null default now()
);
