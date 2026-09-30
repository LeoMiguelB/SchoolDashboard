-- Unified Supabase Database Schema for Academic Dashboard & Dayplanner
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
