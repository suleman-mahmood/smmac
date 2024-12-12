create type Reachability as enum (
	'SAFE',
	'UNKNOWN',
	'RISKY',
	'INVALID'
);

alter table email add column reachability Reachability;
alter table email alter column reachability set not null;
