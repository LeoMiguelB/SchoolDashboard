import json
import glob
import os

vault_path = "/home/lmb/Documents/LeeOsVault/F26"
files = sorted(glob.glob(f"{vault_path}/*/deadlines.deprecated.json"))

out_dir = "/home/lmb/Desktop/projects/School-Dashboard/supabase"
os.makedirs(out_dir, exist_ok=True)

schema_sql = """-- Unified Supabase Database Schema for Academic Dashboard & Dayplanner
-- Generated for PostgreSQL / Supabase

-- 1. Courses Table (School Dashboard)
CREATE TABLE IF NOT EXISTS courses (
    course_code TEXT PRIMARY KEY,
    course_name TEXT NOT NULL,
    instructor TEXT,
    color TEXT,
    tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    links JSONB DEFAULT '{}'::jsonb,
    policies JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT timezone('utc'::text, now()),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT timezone('utc'::text, now())
);

-- 2. Deadlines Table (School Dashboard)
CREATE TABLE IF NOT EXISTS deadlines (
    id TEXT PRIMARY KEY,
    course_code TEXT NOT NULL REFERENCES courses(course_code) ON DELETE CASCADE,
    title TEXT NOT NULL,
    item_type TEXT NOT NULL,
    due_date TIMESTAMPTZ NOT NULL,
    weight DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    status TEXT NOT NULL DEFAULT 'todo',
    submission_platform TEXT,
    lead_time_days BIGINT,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT timezone('utc'::text, now()),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT timezone('utc'::text, now())
);

CREATE INDEX IF NOT EXISTS idx_deadlines_course ON deadlines(course_code);
CREATE INDEX IF NOT EXISTS idx_deadlines_due_date ON deadlines(due_date);
CREATE INDEX IF NOT EXISTS idx_deadlines_status ON deadlines(status);

-- 3. Planner Categories Table (Dayplanner)
CREATE TABLE IF NOT EXISTS planner_categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT timezone('utc'::text, now()),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT timezone('utc'::text, now())
);

-- 4. Planner Time Blocks Table (Dayplanner)
CREATE TABLE IF NOT EXISTS planner_blocks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    category_id TEXT NOT NULL REFERENCES planner_categories(id) ON DELETE CASCADE,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    all_day BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT timezone('utc'::text, now()),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT timezone('utc'::text, now())
);

CREATE INDEX IF NOT EXISTS idx_planner_blocks_time ON planner_blocks(start_time, end_time);

-- 5. Row Level Security (RLS) & Policies
ALTER TABLE courses ENABLE ROW LEVEL SECURITY;
ALTER TABLE deadlines ENABLE ROW LEVEL SECURITY;
ALTER TABLE planner_categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE planner_blocks ENABLE ROW LEVEL SECURITY;

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE tablename = 'courses' AND policyname = 'Allow all access for courses') THEN
        CREATE POLICY "Allow all access for courses" ON courses FOR ALL USING (true) WITH CHECK (true);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE tablename = 'deadlines' AND policyname = 'Allow all access for deadlines') THEN
        CREATE POLICY "Allow all access for deadlines" ON deadlines FOR ALL USING (true) WITH CHECK (true);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE tablename = 'planner_categories' AND policyname = 'Allow all access for planner_categories') THEN
        CREATE POLICY "Allow all access for planner_categories" ON planner_categories FOR ALL USING (true) WITH CHECK (true);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE tablename = 'planner_blocks' AND policyname = 'Allow all access for planner_blocks') THEN
        CREATE POLICY "Allow all access for planner_blocks" ON planner_blocks FOR ALL USING (true) WITH CHECK (true);
    END IF;
END $$;
"""

with open(f"{out_dir}/schema.sql", "w") as fp:
    fp.write(schema_sql)

def sql_escape(s):
    if s is None:
        return "NULL"
    return "'" + s.replace("'", "''") + "'"

seed_lines = [
    "-- Seed data extracted from vault deadlines.deprecated.json",
    "BEGIN;",
    ""
]

courses = []
deadlines = []

for f in files:
    with open(f) as fp:
        c = json.load(fp)
        courses.append(c)
        code = c["course_code"]
        for it in c.get("items", []):
            it["course_code"] = code
            deadlines.append(it)

seed_lines.append("-- Insert Courses")
for c in courses:
    code = sql_escape(c["course_code"])
    name = sql_escape(c["course_name"])
    instructor = sql_escape(c.get("instructor"))
    color = sql_escape(c.get("color"))
    tags = sql_escape(json.dumps(c.get("tags", []))) + "::jsonb"
    links = sql_escape(json.dumps(c.get("links", {}))) + "::jsonb"
    policies = sql_escape(json.dumps(c.get("policies", {}))) + "::jsonb"
    seed_lines.append(f"INSERT INTO courses (course_code, course_name, instructor, color, tags, links, policies) VALUES ({code}, {name}, {instructor}, {color}, {tags}, {links}, {policies}) ON CONFLICT (course_code) DO UPDATE SET course_name = EXCLUDED.course_name, instructor = EXCLUDED.instructor, color = EXCLUDED.color, tags = EXCLUDED.tags, links = EXCLUDED.links, policies = EXCLUDED.policies, updated_at = now();")

seed_lines.append("\n-- Insert Deadlines")
for it in deadlines:
    i_id = sql_escape(it["id"])
    c_code = sql_escape(it["course_code"])
    title = sql_escape(it["title"])
    itype = sql_escape(it["type"])
    due = sql_escape(it["due_date"])
    weight = it.get("weight", 0.0)
    status = sql_escape(it.get("status", "todo"))
    plat = sql_escape(it.get("submission_platform"))
    lead = it.get("lead_time_days") if it.get("lead_time_days") is not None else "NULL"
    notes = sql_escape(it.get("notes"))
    seed_lines.append(f"INSERT INTO deadlines (id, course_code, title, item_type, due_date, weight, status, submission_platform, lead_time_days, notes) VALUES ({i_id}, {c_code}, {title}, {itype}, {due}, {weight}, {status}, {plat}, {lead}, {notes}) ON CONFLICT (id) DO UPDATE SET title = EXCLUDED.title, item_type = EXCLUDED.item_type, due_date = EXCLUDED.due_date, weight = EXCLUDED.weight, status = EXCLUDED.status, submission_platform = EXCLUDED.submission_platform, lead_time_days = EXCLUDED.lead_time_days, notes = EXCLUDED.notes, updated_at = now();")

seed_lines.append("\nCOMMIT;")

with open(f"{out_dir}/seed.sql", "w") as fp:
    fp.write("\n".join(seed_lines))

print(f"Generated {out_dir}/schema.sql and {out_dir}/seed.sql with {len(courses)} courses and {len(deadlines)} deadlines.")
