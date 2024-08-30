// /*
//     Basic example.

//    Uses anonymous types, making it challenging to keep small components.
//    For more nuanced or complex use cases, strong typing should be used.
// */
// pub fn App() -> Element {
//     let users = query! {
//         "SELECT * FROM user;";
//     };

//     rsx! {
//         for user in &query.read() {
//             li {
//                 "User: {user}"
//             }
//         }
//     }

// }

// /*
//     Strongly typed examples.

//     In cases where you want to pass a query value to other functions,
//     prefer this syntax.

//     queryType! will generate multiple types at this scope.
//     By default they are 'pub', making them accessible externally.

//     If 'user' contains a complex field, for example 'address', it will generate
//     a second type, 'UserAddress'.

//     This way, you can write components that take pieces of a larger object,
//     without scope creep.
// */
// pub fn IntelView(id: u64) -> Element {
//     let intel = query! {
//         "LIVE SELECT * FROM ONLY intel:{id};"
//     };

//     rsx! {
//         h1 { "AI Summary of the intel!" }
//         match intel.summary {
//             Some(summary) => p { "{intel.summary}" },
//             None => div {
//                 LoadingWheel {
//                    percentage: intel.currentPage / intel.totalPages
//                 },
//                 h2 { "This document is pending " }
//             }
//         }
//     }
// }

// pub fn TypedApp() -> Element {
//     let users = User.execute();
//     rsx! {
//         for user in &users.read() {
//             li {
//                 "User: {user}"
//             }
//         }
//     }
// }

// #[derive(PartialEq, Props)]
// struct UserProp(User);

// pub fn UserComponent(cx: Scope<UserProps>) -> Element {
//     rsx! {
//         h1 {
//             "Username: {cx.props.username}"
//         }
//         b {
//             "User DOB: {cx.props.dob}"
//         }
//     }
// }

// pub fn AlertInsights -> Element {
//     let alerts = query! {
//         "SELECT count() FROM reports GROUP BY type;"
//     };

// }

// pub fn AnalysisView() -> Element {
//     let todo = query! {
//         "SELECT * FROM intel WHERE verified = false;"
//     };

//     rsx! {
//         table {
//             tr {
//                 th { "Report Name" }
//                 th { "Report Date" }
//                 th { "Source" }
//                 th { "Counter" }
//             }
//             for report in todo {
//                 tr {
//                     class: "h-36 w-2/3"
//                     td { "{report.name}" }
//                     td { "{report.date}" }
//                     td {
//                         onclick: move |event| {
//                             query! {
//                                 "UPDATE {report} SET count += 1;"
//                             };
//                         });
//                         "Count: {report.count}"
//                     }
//                 }
//             }
//         }
//     }
// }

fn main() {}
