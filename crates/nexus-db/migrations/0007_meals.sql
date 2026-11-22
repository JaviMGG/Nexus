-- NEXUS Meal Planner: recetario, planificador semanal, menús plantilla,
-- listas de la compra y despensa.
CREATE TABLE IF NOT EXISTS recipes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    image_file_id TEXT REFERENCES files(id) ON DELETE SET NULL,
    servings INTEGER NOT NULL DEFAULT 2,
    prep_time_min INTEGER NOT NULL DEFAULT 0,
    cook_time_min INTEGER NOT NULL DEFAULT 0,
    difficulty TEXT NOT NULL DEFAULT 'facil',
    categories TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    ingredients_json TEXT NOT NULL DEFAULT '[]',
    steps_json TEXT NOT NULL DEFAULT '[]',
    nutrition_json TEXT,
    favorite INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_recipes_name ON recipes(name);
CREATE INDEX IF NOT EXISTS idx_recipes_favorite ON recipes(favorite);

-- Entradas del planificador semanal. week_start es siempre un lunes YYYY-MM-DD.
CREATE TABLE IF NOT EXISTS meal_plan_entries (
    id TEXT PRIMARY KEY,
    week_start TEXT NOT NULL,
    date TEXT NOT NULL,
    slot TEXT NOT NULL,
    recipe_id TEXT NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    servings INTEGER NOT NULL DEFAULT 2,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_meal_plan_week ON meal_plan_entries(week_start);
CREATE INDEX IF NOT EXISTS idx_meal_plan_recipe ON meal_plan_entries(recipe_id);

-- Menús personalizados: semanas guardadas como plantilla reutilizable.
CREATE TABLE IF NOT EXISTS menu_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    entries_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Listas de la compra (generadas desde una semana o creadas a mano).
CREATE TABLE IF NOT EXISTS shopping_lists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    week_start TEXT,
    has_manual_changes INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_shopping_lists_created ON shopping_lists(created_at);

CREATE TABLE IF NOT EXISTS shopping_items (
    id TEXT PRIMARY KEY,
    list_id TEXT NOT NULL REFERENCES shopping_lists(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    name_key TEXT NOT NULL,
    quantity REAL NOT NULL DEFAULT 0,
    unit TEXT NOT NULL DEFAULT 'ud',
    category TEXT NOT NULL DEFAULT 'otros',
    checked INTEGER NOT NULL DEFAULT 0,
    manual INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_shopping_items_list ON shopping_items(list_id);

-- Despensa: inventario con caducidades introducidas por el usuario.
CREATE TABLE IF NOT EXISTS pantry_items (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    name_key TEXT NOT NULL,
    quantity REAL NOT NULL DEFAULT 0,
    unit TEXT NOT NULL DEFAULT 'ud',
    category TEXT NOT NULL DEFAULT 'otros',
    expires_at TEXT,
    location TEXT NOT NULL DEFAULT 'despensa',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_pantry_name_key ON pantry_items(name_key);
CREATE INDEX IF NOT EXISTS idx_pantry_location ON pantry_items(location);
