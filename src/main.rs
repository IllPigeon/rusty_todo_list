use cursive::utils::span::SpannedString;
use cursive::Cursive;
use cursive::view::{Nameable, Resizable, Scrollable};
use cursive::views::{TextView, Button, Dialog, EditView, LinearLayout, SelectView};
use cursive_async_view::{AsyncProgressView, AsyncProgressState};
use rusqlite::{params, Connection, Result};
use std::{fs, thread, time};
use cursive::utils::markup::StyledString;





/** Used for storing todo list task data */
struct Task {
    name: String,
    completed: bool
}



/** This code is for a CLI to-do list built entirely in Rust with a functioning sqlite database locally on a machine.
 * The CLI was built using the Cursive crate to build views, and rusqlite was used for database operations.
 * Purpose of this project was to become familiar with Rust's crate ecosystem.
 */
fn main() {
    // main cursive instance
    let mut siv = cursive::default();
    siv.add_global_callback('q', |s| s.quit());
    // connection and path of database, connection is needed for database creationa & transactions
    let db_path = "./src/resources/db/tasks.db";
    let conn = Connection::open(db_path).expect("Failed to open the database");
    create_table(&conn).expect("Error initializing database");
    // Retrieving data as vector to add into view
    let task_list = retrieve_list(&conn);
    // very important for keeping single instance of database connection to be passed in different functions
    siv.set_user_data(conn);

    let start = time::Instant::now();
    let async_view = AsyncProgressView::new(&mut siv, move || {
        if start.elapsed().as_secs() < 5 {
            AsyncProgressState::Pending(start.elapsed().as_secs_f32() / 5f32)
        } 
        else {
            // Creating view to populate with clone of fetched data of tasks, plain text is data used for database operations, styled task is how its presented visually
            let mut tasks_view = SelectView::<String>::new();
            for styled_task in task_list.clone() {
                let plain_task = styled_task.source().to_string();
                tasks_view.add_item(styled_task, plain_task);
            }

            let tasks = tasks_view
                .on_submit(set_status)
                .with_name("tasks")
                .scrollable()
                .fixed_size((35, 12));

            let buttons = LinearLayout::horizontal()
                .child(Button::new("Add", add_todo))
                .child(Button::new("Delete", remove_todo));
            AsyncProgressState::Available(Dialog::around(LinearLayout::vertical()
                .child(tasks)
                .child(buttons))
            )
        }
    });
    siv.add_layer(Dialog::around(async_view).title("Rusty To-Do List"));
    siv.run();
}


/** Used for creating the database of tasks for the todo list */
fn create_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
            name TEXT PRIMARY KEY,
            completed BOOLEAN
    )", [])?;
    return Ok(());
}


/** Used for retrieving todo list data to be displayed in the cursive view with styling data based on completion*/
fn retrieve_list(conn: &Connection) -> Vec<StyledString> {
    let mut result_vec: Vec<StyledString> = Vec::new();
    let mut stmt = conn.prepare("SELECT name, completed FROM tasks").expect("Error retrieving tasks from database");

    let task_iter = stmt.query_map([], |row| {
        Ok(Task {
            // task name is tied to column 0, completion state is tied to column 1
            name: row.get(0)?,
            completed: row.get(1)?
        })
    });

    for task in task_iter.expect("Failed to query tasks") {
        let unwrapped_task = task.unwrap();
        if !unwrapped_task.completed {
            let unfin_task = SpannedString::styled(
                unwrapped_task.name, 
                cursive::style::Effect::Simple);
            result_vec.push(unfin_task);
        }
        else {
            let fin_task = SpannedString::styled(
                unwrapped_task.name, 
                cursive::style::Effect::Strikethrough);
            result_vec.push(fin_task);
        }
    }
    return result_vec;
}


/** Used for adding tasks to the todo list */
fn add_todo(s: &mut Cursive) {

    // Used for inserting a todo list item into the database
    fn insert_data(conn: &Connection, task_name: &str) -> Result<()> {
        conn.execute("INSERT INTO tasks (name, completed) VALUES (?1, ?2)", params![task_name, false])?;
        Ok(())
    }

    // Nested function for submission of adding another item
    fn ok(s: &mut Cursive, task_name: &str) {
        s.call_on_name("tasks", |view: &mut SelectView<String>| {
            view.add_item_str(task_name);
        });
        s.with_user_data(|conn: &mut Connection| {
            insert_data(conn, task_name).expect("Failed to insert item");
        });
        s.pop_layer();
    }

    s.add_layer(Dialog::around(EditView::new()
        .on_submit(ok)
        .with_name("task")
        .fixed_width(28))
    .title("Enter task name")
    .button("Ok", |s| {
        let task = s.call_on_name("task", |view: &mut EditView| {
            view.get_content()
        }).unwrap();
        ok(s, &task);
        
    })
    .button("Cancel", |s| {
        s.pop_layer();
    }));
}


/** Used for removing a todo task */
fn remove_todo(s: &mut Cursive) {
    
    // Nested function for deleting task from database
    fn delete_data(conn: &Connection, task_data: &String) {
        conn.execute("DELETE FROM tasks WHERE (name) IS (?1)", [task_data]).expect("Error removing task");
    }

    // get all tasks from the select view
    let mut tasks = s.find_name::<SelectView<String>>("tasks").unwrap();
    // match the tasks based on the selected id, if the focus matches selected id remove the item
    match tasks.selected_id(){
        None => s.add_layer(Dialog::info("No task to remove")),
        Some(focus) => {
            let task_data = tasks.get_item(focus).map(|(_, data)| data.clone()).expect("Failed to access task data for deletion");
            tasks.remove_item(focus);
            s.with_user_data(|conn: &mut Connection| {
                delete_data(conn, &task_data);
            });
        }
    }
}


/** Used for updating status of a task to either be completed or incomplete */
fn set_status(s: &mut Cursive, task: &str) {

    // Nested function for retrieving status
    fn get_status(conn: &Connection, task: &str) -> bool {
        return conn.query_row("SELECT completed FROM tasks WHERE name = ?1", [task], |row| row.get(0)).unwrap_or(false);
    }
    
    // Nested function for updating status
    fn update_status(conn: &Connection, task: &str, status: bool) {
        conn.execute("UPDATE tasks SET completed = ?2 WHERE name IS ?1", params![task, !status]).expect("Error updating task status");
    }

    let mut tasks: cursive::views::ViewRef<SelectView> = s.find_name::<SelectView<String>>("tasks").unwrap();
    if let Some(id) = tasks.selected_id() {
        let task_data = tasks.get_item(id).map(|(_, data) | data.clone());
        tasks.remove_item(id);
        if let Some(data) = task_data {
            // Using connection that is stored in view to retrieve selected task status, then update it.
            s.with_user_data(|conn: &mut Connection| {
                let task_status = get_status(conn, task);
                update_status(conn, task, task_status);
                // If the task was false when selected, update it to finished since it was set to true with update and vice versa
                if !task_status {
                    let fin_task = SpannedString::styled(task, cursive::style::Effect::Strikethrough);
                    tasks.insert_item(id, fin_task, data);
                } 
                else {
                    let unfin_task = SpannedString::styled(task, cursive::style::Effect::Simple);
                    tasks.insert_item(id, unfin_task, data);
                }
            });
        }
        tasks.set_selection(id);
    }
}


