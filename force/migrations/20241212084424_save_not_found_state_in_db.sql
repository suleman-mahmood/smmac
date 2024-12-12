alter table product add column no_results bool;
alter table product alter column no_results set not null;
alter table product alter column no_results set default false;

alter table founder add column no_results bool;
alter table founder alter column no_results set not null;
alter table founder alter column no_results set default false;
