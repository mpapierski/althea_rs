import React, { Component } from "react";
import logo from "./CascadianMesh.png";
import "./App.css";
import { Button, Table } from "reactstrap";

const statServer = "http://127.0.0.1:8080/mock.json";

class App extends Component {
  constructor(props) {
    super(props);
    this.state = { nodes: [] };
  }

  componentDidMount() {
    this.timerID = setInterval(() => this.updateNodes(), 1000);
  }

  componentWillUnmount() {
    clearInterval(this.timerID);
  }

  async updateNodes() {
    const rand = Math.random();
    console.log("req", rand);
    const nodes = await (await fetch(statServer)).json();
    console.log("res", rand, nodes);
    this.setState({ nodes });
  }

  render() {
    return (
      <div className="App">
        <header className="App-header">
          <img src={logo} className="App-logo" alt="logo" />
          <h1 className="App-title">CascadianMesh hypothetical balances</h1>
        </header>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center"
          }}
        >
          <Table
            striped
            style={{ textAlign: "left", maxWidth: 600, marginTop: 50 }}
          >
            <thead>
              <tr>
                <th>IP address</th>
                <th>MAC address</th>
                <th>Balance</th>
              </tr>
            </thead>
            <tbody>
              {this.state.nodes.map((node, i) => (
                <tr key={i}>
                  <td>
                    <b>{node.ip}</b>
                  </td>
                  <td>{node.mac}</td>
                  <td>{node.balance}</td>
                </tr>
              ))}
            </tbody>
          </Table>
        </div>
      </div>
    );
  }
}

export default App;
